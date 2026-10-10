//! The embedding model: download, and replacing the one shipped before,
//! SPEC 5.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::embed::activity::Activity;
use crate::embed::download;
use crate::embed::model::{self, models_dir, EmbeddingModel, LEGACY_EMBEDDING_FILE};
use crate::state::AppState;

/// What the settings screen shows about the embedding model.
#[derive(Debug, Serialize)]
pub struct EmbeddingModelInfo {
    pub installed: bool,
    /// How far the download is, while one runs.
    pub downloading: Option<u8>,
}

#[tauri::command]
pub async fn embedding_model_info(
    state: State<'_, AppState>,
) -> Result<EmbeddingModelInfo, String> {
    Ok(EmbeddingModelInfo {
        installed: download::is_installed(&state.local_data, EmbeddingModel),
        downloading: state.embedding_download.lock().ok().and_then(|d| *d),
    })
}

/// What the embedder is doing, for the status bar of a dev build.
#[tauri::command]
pub fn embedder_activity(state: State<'_, AppState>) -> Activity {
    state
        .embedder_activity
        .lock()
        .map(|activity| activity.clone())
        .unwrap_or_default()
}

/// Fetch the embedding model, behind search by meaning, similar notes,
/// recall and threads. The embed task is woken once it is in, loads it and
/// embeds the notes already there. Progress goes out as `embedding-status`.
#[tauri::command]
pub async fn download_embedding_model(app: AppHandle) -> Result<(), String> {
    fetch_embedding_model(&app).await
}

/// `download_embedding_model`, also started for an install that still has
/// the model shipped before.
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
        download::fetch(&state.local_data, EmbeddingModel, &remote, move |percent| {
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

/// The chat models a version before 0.5.0 left in `models/`, which nothing
/// reads any more, and how much room they take.
#[derive(Debug, Serialize)]
pub struct OldChatModel {
    pub bytes: u64,
}

/// The old chat models still on disk, if any, for the main window to offer
/// removing them.
#[tauri::command]
pub async fn old_chat_model(state: State<'_, AppState>) -> Result<Option<OldChatModel>, String> {
    Ok(model::old_chat_bytes(&state.local_data).map(|bytes| OldChatModel { bytes }))
}

/// Remove the old chat models, once the user said so.
#[tauri::command]
pub async fn remove_old_chat_model(state: State<'_, AppState>) -> Result<(), String> {
    model::remove_old_chat_models(&state.local_data)
}

/// An install from before EmbeddingGemma still has Qwen3-Embedding on disk.
/// Its successor is fetched in the background, so search by meaning comes
/// back without a trip to settings, and the old file goes once the new one is
/// in.
pub fn replace_legacy_embedding_model(app: &AppHandle) {
    let local_data = app.state::<AppState>().local_data.clone();
    let legacy = models_dir(&local_data).join(LEGACY_EMBEDDING_FILE);
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
    if download::is_installed(&local_data, EmbeddingModel) {
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
