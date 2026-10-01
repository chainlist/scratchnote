//! The model and the embedding model: download, load, update and
//! benchmark, SPEC 5.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use super::settings::{current_settings, save_settings};
use crate::enrich::download::{self, RemoteModel};
use crate::enrich::model::{
    model_file, models_dir, EmbeddingModel, ModelStatus, Variant, LEGACY_EMBEDDING_FILE,
};
use crate::state::AppState;

#[tauri::command]
pub fn model_status(state: State<'_, AppState>) -> ModelStatus {
    state.model_status()
}

/// Whether the worker is on a job right now; `enrich-busy` reports changes.
#[tauri::command]
pub fn enrich_busy(state: State<'_, AppState>) -> bool {
    state.busy.load(std::sync::atomic::Ordering::SeqCst)
}

/// Which note of how many the worker is on; `enrich-progress` reports changes.
#[tauri::command]
pub fn enrich_progress(state: State<'_, AppState>) -> Option<crate::state::Progress> {
    state.progress()
}

/// What the benchmark sends the page as it runs.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum BenchmarkEvent {
    /// The model was not in memory; this is what loading it took.
    Load { ms: u64 },
    /// Sample `done` of `total` is labelled, or failed to be.
    Note {
        done: usize,
        total: usize,
        ms: u64,
        ok: bool,
    },
}

/// Label the benchmark's sample notes with the model in use, loading it if
/// need be, and time each one, reported through `on_event`. Nothing is
/// written anywhere.
#[tauri::command]
pub async fn benchmark_model(
    app: AppHandle,
    state: State<'_, AppState>,
    on_event: tauri::ipc::Channel<BenchmarkEvent>,
) -> Result<(), String> {
    use crate::enrich::benchmark::SAMPLES;
    use std::time::Instant;

    let space = state.space()?;
    // Loading and inference block, so none of it may run on the async runtime.
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let resident = state.backend.read().is_ok_and(|slot| slot.is_some());
        let started = Instant::now();
        let backend = crate::enrich::worker::backend(&app)
            .ok_or_else(|| "There is no model to benchmark.".to_string())?;
        if !resident {
            let ms = started.elapsed().as_millis() as u64;
            let _ = on_event.send(BenchmarkEvent::Load { ms });
        }

        let language = state.note_language();
        let categories = space.categories();
        for (i, sample) in SAMPLES.iter().enumerate() {
            // Keeps the idle unload away for as long as the benchmark runs.
            state.mark_used();
            let started = Instant::now();
            let result =
                crate::enrich::runner::enrich(sample, language, &categories, backend.as_ref());
            let ms = started.elapsed().as_millis() as u64;
            if let Err(e) = &result {
                log::warn!("benchmark sample {i} failed: {e}");
            }
            let _ = on_event.send(BenchmarkEvent::Note {
                done: i + 1,
                total: SAMPLES.len(),
                ms,
                ok: result.is_ok(),
            });
        }
        state.mark_used();
        Ok(())
    })
    .await
    .map_err(|e| format!("the benchmark failed: {e}"))?
}

/// What the settings screen shows about models: the one in use, and the
/// record of each variant the app has fetched.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub active_path: Option<std::path::PathBuf>,
    /// `None` for a custom GGUF file.
    pub active_variant: Option<Variant>,
    pub default: Option<download::InstalledModel>,
    pub light: Option<download::InstalledModel>,
}

/// GPUs the model could run on, for the settings toggle. Empty means the
/// machine has none the build can use, so the model runs on the CPU.
#[tauri::command]
pub fn gpu_devices() -> Vec<String> {
    crate::enrich::llama::gpu_devices()
}

/// The machine as the onboarding sees it, and the model it suggests, before
/// any model is on disk for the benchmark to time.
#[tauri::command]
pub fn system_profile() -> crate::enrich::hardware::Hardware {
    crate::enrich::hardware::probe()
}

#[tauri::command]
pub fn model_info(state: State<'_, AppState>) -> ModelInfo {
    let active = state.active_model();
    ModelInfo {
        active_path: active.as_ref().map(|a| a.path.clone()),
        active_variant: active.and_then(|a| a.variant),
        default: download::installed(&state.root, Variant::Default),
        light: download::installed(&state.root, Variant::Light),
    }
}

/// Fetch a model, make it the one in use, and load it. Capture keeps working
/// throughout; notes simply stay pending until this finishes (SPEC 5.2).
#[tauri::command]
pub async fn download_model(
    app: AppHandle,
    state: State<'_, AppState>,
    variant: Variant,
) -> Result<(), String> {
    let root = state.root.clone();
    let remote: RemoteModel = download::lookup(variant).await?;

    // Fetching a second variant leaves the first one working meanwhile, so a
    // failure goes back to that rather than to "absent".
    let before = state.model_status();
    set_status(&app, &state, ModelStatus::Downloading { percent: 0 });
    let progress_app = app.clone();
    let result = download::fetch(&root, variant, &remote, move |percent| {
        let _ = progress_app.emit(
            "model-status",
            serde_json::json!({ "state": "downloading", "percent": percent }),
        );
    })
    .await;

    if let Err(e) = result {
        set_status(&app, &state, before);
        return Err(e);
    }

    // Search and similar notes need the embedding model, so it comes along
    // with the first chat model rather than waiting on a trip to settings.
    // Only once the chat model is in, which enrichment needs first.
    let embedding_running = state.embedding_download.lock().is_ok_and(|d| d.is_some());
    if !embedding_running && !download::is_installed(&root, EmbeddingModel) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = fetch_embedding_model(&app).await {
                log::warn!("the embedding model did not download: {e}");
            }
        });
    }

    let mut settings = current_settings(&state)?;
    settings.model_variant = variant;
    settings.model_path = None;
    save_settings(&app, &state, settings).await?;
    load_model(&app, &state, variant)
}

/// Load a model that is already on disk. Lazy loading on the first job is the
/// normal path (SPEC 5.1); this is the eager one, after a download.
pub fn load_model(
    app: &AppHandle,
    state: &State<'_, AppState>,
    variant: Variant,
) -> Result<(), String> {
    // A download while the model is off leaves it on disk for later.
    if !state.model_enabled() {
        set_status(app, state, ModelStatus::Disabled);
        return Ok(());
    }
    let path = model_file(&state.root, variant);
    // After a lazy load under way, not beside it.
    let _one_at_a_time = state
        .backend_loading
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let backend = crate::enrich::llama::LlamaCpp::load_with(&path, state.use_gpu())?;

    if let Ok(mut slot) = state.backend.write() {
        *slot = Some(std::sync::Arc::new(backend));
    }
    // Otherwise the idle unload counts from the last job, however long ago.
    state.mark_used();
    set_status(app, state, ModelStatus::Loaded);
    // Anything that was waiting on a model can go now.
    state.wake.notify_one();
    state.embed_wake.notify_one();
    Ok(())
}

/// What the settings screen shows about the embedding model.
#[derive(Debug, Serialize)]
pub struct EmbeddingModelInfo {
    pub installed: bool,
    /// How far the download is, while one runs.
    pub downloading: Option<u8>,
}

#[tauri::command]
pub fn embedding_model_info(state: State<'_, AppState>) -> EmbeddingModelInfo {
    EmbeddingModelInfo {
        installed: download::is_installed(&state.root, EmbeddingModel),
        downloading: state.embedding_download.lock().ok().and_then(|d| *d),
    }
}

/// Fetch the embedding model, which lets the chat find the notes a question
/// is about. It is not a chat model: no setting changes and the chat model
/// is left alone. The embed task is woken instead, loads it and embeds the
/// notes already there. Progress goes out as `embedding-status`.
#[tauri::command]
pub async fn download_embedding_model(app: AppHandle) -> Result<(), String> {
    fetch_embedding_model(&app).await
}

/// `download_embedding_model`, also started by `download_model`.
async fn fetch_embedding_model(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        let mut running = state
            .embedding_download
            .lock()
            .map_err(|_| "download lock poisoned".to_string())?;
        if running.is_some() {
            return Err("the embedding model is already downloading".to_string());
        }
        *running = Some(0);
    }
    let _ = app.emit(
        "embedding-status",
        serde_json::json!({ "state": "downloading", "percent": 0 }),
    );

    let progress_app = app.clone();
    let result = async {
        let remote = download::lookup(EmbeddingModel).await?;
        download::fetch(&state.root, EmbeddingModel, &remote, move |percent| {
            if let Ok(mut running) = progress_app.state::<AppState>().embedding_download.lock() {
                *running = Some(percent);
            }
            let _ = progress_app.emit(
                "embedding-status",
                serde_json::json!({ "state": "downloading", "percent": percent }),
            );
        })
        .await
    }
    .await;

    if let Ok(mut running) = state.embedding_download.lock() {
        *running = None;
    }
    let status = if result.is_ok() {
        // The notes already there get embedded now.
        state.embed_wake.notify_one();
        "installed"
    } else {
        "absent"
    };
    let _ = app.emit("embedding-status", serde_json::json!({ "state": status }));
    result
}

/// An install from before EmbeddingGemma still has Qwen3-Embedding on disk.
/// Its successor is fetched in the background, so search by meaning comes
/// back without a trip to settings, and the old file goes once the new one is
/// in.
pub fn replace_legacy_embedding_model(app: &AppHandle) {
    let root = app.state::<AppState>().root.clone();
    let legacy = models_dir(&root).join(LEGACY_EMBEDDING_FILE);
    if !legacy.exists() {
        return;
    }
    let remove_legacy = move || {
        for path in [legacy.with_extension("gguf.json"), legacy.clone()] {
            if let Err(e) = std::fs::remove_file(&path) {
                log::warn!("could not remove {}: {e}", path.display());
            }
        }
    };
    if download::is_installed(&root, EmbeddingModel) {
        remove_legacy();
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match fetch_embedding_model(&app).await {
            Ok(()) => remove_legacy(),
            Err(e) => log::warn!("the new embedding model did not download: {e}"),
        }
    });
}

/// Drop the chat model, which is how a model switch or a GPU change takes
/// effect: the next job loads whatever the settings now point at. The
/// embedding model stays loaded.
pub(super) fn unload_chat_model(app: &AppHandle, state: &State<'_, AppState>) {
    let status = state.unload_chat_model();
    let _ = app.emit("model-status", &status);
}

fn set_status(app: &AppHandle, state: &State<'_, AppState>, status: ModelStatus) {
    state.set_model_status(status.clone());
    let _ = app.emit("model-status", &status);
}

/// The app's own variant in use, with its record. A custom GGUF file has no
/// revision to compare, so it is never checked (SPEC 5.2).
fn checkable_model(
    state: &State<'_, AppState>,
) -> Result<(Variant, download::InstalledModel), String> {
    let active = state
        .active_model()
        .ok_or_else(|| "no model is installed".to_string())?;
    let variant = active
        .variant
        .ok_or_else(|| "a custom GGUF file is not checked for updates".to_string())?;
    let installed = download::installed(&state.root, variant)
        .ok_or_else(|| "the installed model has no record of its revision".to_string())?;
    Ok((variant, installed))
}

/// SPEC 5.2: only ever run from the settings button. A failure is reported,
/// never raised, and has no effect on enrichment. The `Result` is only there
/// because Tauri requires one of async commands; it is always `Ok`.
#[tauri::command]
pub async fn check_model_update(
    state: State<'_, AppState>,
) -> Result<download::UpdateCheck, String> {
    let (variant, installed) = match checkable_model(&state) {
        Ok(found) => found,
        Err(reason) => return Ok(download::UpdateCheck::Failed { reason }),
    };
    Ok(match download::lookup(variant).await {
        Ok(remote) => download::compare(&installed, &remote),
        Err(reason) => download::UpdateCheck::Failed { reason },
    })
}

/// Fetch the current revision of the model in use and swap it in. The old
/// file stays loaded and in use until the new one has downloaded and passed
/// its hash (SPEC 5.2).
#[tauri::command]
pub async fn update_model(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    use std::sync::atomic::Ordering;

    let (variant, installed) = checkable_model(&state)?;
    let remote = download::lookup(variant).await?;
    if remote.revision == installed.revision {
        return Ok(());
    }

    let progress_app = app.clone();
    download::download_verified(&state.root, variant, &remote, move |percent| {
        let _ = progress_app.emit("model-update", serde_json::json!({ "percent": percent }));
    })
    .await?;

    // Windows will not replace a file that a loaded model has mapped, so let
    // go of it, and give a job still holding it a moment to finish. Taking
    // the load lock first lets a load under way finish, so it is let go of
    // too; loads after it read the flag under the same lock and stay off.
    {
        let _one_at_a_time = state
            .backend_loading
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.swapping.store(true, Ordering::SeqCst);
        unload_chat_model(&app, &state);
    }
    let mut result = Err(String::new());
    for _ in 0..30 {
        result = download::install(&state.root, variant, &remote).await;
        if result.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    state.swapping.store(false, Ordering::SeqCst);
    state.wake.notify_one();
    state.embed_wake.notify_one();

    result.map_err(|e| {
        format!(
            "the new model is downloaded and verified but could not replace the old one ({e}). \
             Try again in a moment; it will not download again."
        )
    })
}
