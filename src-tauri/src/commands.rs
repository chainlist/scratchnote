//! Tauri commands, SPEC 8.

use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use ulid::Ulid;

use crate::enrich::download::{self, RemoteModel};
use crate::enrich::model::{model_file, ModelStatus, Variant};
use crate::enrich::normalize;
use crate::enrich::queue::{queue_path, Job};
use crate::settings::Settings;
use crate::state::AppState;
use crate::storage::daily_file::{self, Note, NotePatch, Status};
use crate::storage::index::{self, IndexEntry};
use crate::storage::tags;
use crate::storage::{check_date, day_path, relative_day_path};

#[derive(Debug, Serialize)]
pub struct DaySummary {
    pub date: String,
    pub count: usize,
}

/// Append a note to today's file and tell the rest of the app about it.
/// An empty body is a no-op, not an error.
#[tauri::command]
pub async fn save_note(
    app: AppHandle,
    state: State<'_, AppState>,
    body: String,
) -> Result<Option<Note>, String> {
    let body = body.trim().to_string();
    if body.is_empty() {
        return Ok(None);
    }

    let now = Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let note = Note {
        id: Ulid::generate().to_string(),
        time: now.format("%H:%M").to_string(),
        file: relative_day_path(&date),
        subject: None,
        summary: None,
        tags: Vec::new(),
        status: Status::Pending,
        hash: daily_file::body_hash(&body),
        body,
        date: date.clone(),
    };

    state
        .writer
        .append_note(day_path(&state.root, &date), date, note.clone())
        .await?;

    // A new note only ever adds a line, so the index is appended rather than
    // rewritten. This is the capture path and it has a latency budget.
    let entry = IndexEntry::from(&note);
    let line = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
    state
        .index
        .write()
        .map_err(|_| "index lock poisoned".to_string())?
        .push(entry);
    state
        .writer
        .append_index_line(index::index_path(&state.root), line)
        .await?;

    enqueue(&state, note.id.clone(), note.date.clone()).await;

    let _ = app.emit("note-updated", serde_json::json!({ "id": note.id }));
    Ok(Some(note))
}

/// Queue a note for enrichment and nudge the worker. Already-queued notes are
/// left alone, so saving twice does not enrich twice.
async fn enqueue(state: &State<'_, AppState>, id: String, date: String) {
    let queued = match state.queue.lock() {
        Ok(mut queue) => queue.push(Job::new(id, date)),
        Err(_) => false,
    };
    if !queued {
        return;
    }
    persist_queue(state).await;
    state.wake.notify_one();
}

async fn persist_queue(state: &State<'_, AppState>) {
    let contents = match state.queue.lock() {
        Ok(queue) => queue.to_json(),
        Err(_) => return,
    };
    if let Err(e) = state
        .writer
        .write_index(queue_path(&state.root), contents)
        .await
    {
        log::warn!("could not persist the queue: {e}");
    }
}

/// Read straight from the markdown, because the index deliberately carries no
/// bodies and the day view shows them.
#[tauri::command]
pub async fn get_day(state: State<'_, AppState>, date: String) -> Result<Vec<Note>, String> {
    check_date(&date)?;
    let path = day_path(&state.root, &date);
    let contents = match tokio::fs::read_to_string(&path).await {
        Ok(contents) => contents,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut notes = daily_file::parse_notes(&contents, &date, &relative_day_path(&date));
    notes.sort_by(|a, b| a.time.cmp(&b.time));
    Ok(notes)
}

#[tauri::command]
pub async fn list_days(state: State<'_, AppState>) -> Result<Vec<DaySummary>, String> {
    let days = state
        .index
        .read()
        .map_err(|_| "index lock poisoned".to_string())?
        .days();
    Ok(days
        .into_iter()
        .map(|(date, count)| DaySummary { date, count })
        .collect())
}

/// Remove a note from its day file.
///
/// The spec's signature is `delete_note(id)`, which needs the index to resolve
/// an id to a file. The caller passes the date it already has rather than
/// making this search every day in the index.
#[tauri::command]
pub async fn delete_note(
    app: AppHandle,
    state: State<'_, AppState>,
    date: String,
    id: String,
) -> Result<(), String> {
    check_date(&date)?;
    let path = day_path(&state.root, &date);
    if !state.writer.delete_note(path.clone(), id.clone()).await? {
        return Err(format!("no note {id} in {date}"));
    }

    // A delete rewrites the day file, so the day's entries are reparsed and
    // the whole index written back (SPEC 4.4).
    let entries = index::parse_day(&path, &date);
    state
        .index
        .write()
        .map_err(|_| "index lock poisoned".to_string())?
        .replace_day(&date, entries);
    state.persist_index().await?;

    if let Ok(mut queue) = state.queue.lock() {
        queue.remove(&id);
    }
    persist_queue(&state).await;

    let _ = app.emit("note-updated", serde_json::json!({ "id": id }));
    Ok(())
}

/// Edit a note's body in the app. A changed body goes back to the model,
/// unless the user has taken the note over (SPEC 4.2).
#[tauri::command]
pub async fn update_note(
    app: AppHandle,
    state: State<'_, AppState>,
    date: String,
    id: String,
    body: String,
) -> Result<Note, String> {
    check_date(&date)?;
    let body = body.trim().to_string();
    if body.is_empty() {
        return Err("a note cannot be empty; delete it instead".to_string());
    }
    let current = read_note(&state, &date, &id).await?;
    if current.body == body {
        return Ok(current);
    }

    let path = day_path(&state.root, &date);
    if !state.writer.replace_body(path, id.clone(), body).await? {
        return Err(format!("no note {id} in {date}"));
    }
    reindex_day(&state, &date).await?;
    if current.status != Status::Manual {
        enqueue(&state, id.clone(), date.clone()).await;
    }

    let _ = app.emit("note-updated", serde_json::json!({ "id": id }));
    read_note(&state, &date, &id).await
}

/// Set a note's subject and tags by hand. The note becomes `manual`, which
/// enrichment never overwrites (SPEC 4.2). `None` keeps the current value.
#[tauri::command]
pub async fn update_note_meta(
    app: AppHandle,
    state: State<'_, AppState>,
    date: String,
    id: String,
    subject: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<Note, String> {
    check_date(&date)?;
    let current = read_note(&state, &date, &id).await?;

    let subject = match subject {
        // The subject is the note's heading, so it has to stay on one line.
        Some(raw) => {
            Some(raw.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|s| !s.is_empty())
        }
        None => current.subject.clone(),
    };
    let tags = match tags {
        Some(raw) => normalize::manual(&raw, &state.vocabulary()),
        None => current.tags.clone(),
    };
    // The file keeps a summary and its tags as one two-line block, and that
    // block cannot be written with the tag line empty.
    if tags.is_empty() && current.summary.is_some() {
        return Err("keep at least one tag".to_string());
    }

    let patch = NotePatch {
        subject,
        summary: current.summary.clone(),
        tags,
        status: Status::Manual,
    };
    let path = day_path(&state.root, &date);
    if !state.writer.update_note(path, id.clone(), patch).await? {
        return Err(format!("no note {id} in {date}"));
    }
    reindex_day(&state, &date).await?;

    // Nothing left for the model to do with it.
    if let Ok(mut queue) = state.queue.lock() {
        queue.remove(&id);
    }
    persist_queue(&state).await;

    let _ = app.emit("note-updated", serde_json::json!({ "id": id }));
    read_note(&state, &date, &id).await
}

async fn read_note(state: &State<'_, AppState>, date: &str, id: &str) -> Result<Note, String> {
    let contents = tokio::fs::read_to_string(day_path(&state.root, date))
        .await
        .map_err(|e| e.to_string())?;
    daily_file::parse_notes(&contents, date, &relative_day_path(date))
        .into_iter()
        .find(|note| note.id == id)
        .ok_or_else(|| format!("no note {id} in {date}"))
}

/// After rewriting a day file: reparse the day and write the index back.
async fn reindex_day(state: &State<'_, AppState>, date: &str) -> Result<(), String> {
    let entries = index::parse_day(&day_path(&state.root, date), date);
    state
        .index
        .write()
        .map_err(|_| "index lock poisoned".to_string())?
        .replace_day(date, entries);
    state.persist_index().await
}

/// Reparse every markdown file and replace the cache. Returns how many notes
/// the rebuilt index holds.
#[tauri::command]
pub async fn rebuild_index(app: AppHandle, state: State<'_, AppState>) -> Result<usize, String> {
    let root = state.root.clone();
    let rebuilt = tauri::async_runtime::spawn_blocking(move || index::rebuild(&root))
        .await
        .map_err(|e| e.to_string())?;

    let count = rebuilt.len();
    *state
        .index
        .write()
        .map_err(|_| "index lock poisoned".to_string())? = rebuilt;
    // Aliases are hand-edited in tags.json, so a rebuild is also when edits
    // made there are picked up.
    if let Ok(mut aliases) = state.aliases.write() {
        *aliases = tags::load_aliases(&state.root);
    }
    state.persist_index().await?;

    let _ = app.emit("index-rebuilt", ());
    Ok(count)
}

/// Every tag in use with how many notes carry it, most used first.
#[tauri::command]
pub fn list_tags(state: State<'_, AppState>) -> Result<Vec<(String, u32)>, String> {
    let counts = state
        .index
        .read()
        .map_err(|_| "index lock poisoned".to_string())?
        .tag_counts();
    Ok(tags::by_count(&counts))
}

/// Words and `#tag` filters across every day (SPEC 6).
#[tauri::command]
pub fn search(state: State<'_, AppState>, query: String) -> Result<Vec<Note>, String> {
    let aliases = state.aliases();
    let idx = state
        .index
        .read()
        .map_err(|_| "index lock poisoned".to_string())?;
    Ok(crate::search::search(&idx, &query, &aliases))
}

/// Settings as saved, plus the root this run is actually using, so the screen
/// can say when a new root is waiting on a restart.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    #[serde(flatten)]
    pub settings: Settings,
    pub active_root: std::path::PathBuf,
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<SettingsView, String> {
    let settings = state
        .settings
        .read()
        .map_err(|_| "settings lock poisoned".to_string())?
        .clone();
    Ok(SettingsView {
        settings,
        active_root: state.root.clone(),
    })
}

/// Save settings (SPEC 7). The hotkey is rebound straight away; a new notes
/// root is saved but only used from the next launch.
#[tauri::command]
pub async fn set_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<SettingsView, String> {
    settings.validate()?;
    let old_hotkey = state
        .settings
        .read()
        .map_err(|_| "settings lock poisoned".to_string())?
        .capture_hotkey
        .clone();

    if settings.capture_hotkey != old_hotkey {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        let shortcuts = app.global_shortcut();
        // Bind the new one first, so a key the OS refuses leaves the old one
        // working instead of leaving no hotkey at all.
        shortcuts
            .register(settings.capture_hotkey.as_str())
            .map_err(|e| format!("cannot use {} as the hotkey: {e}", settings.capture_hotkey))?;
        let _ = shortcuts.unregister(old_hotkey.as_str());
    }

    state
        .writer
        .write_index(crate::settings::file(&app), settings.to_json())
        .await?;
    *state
        .settings
        .write()
        .map_err(|_| "settings lock poisoned".to_string())? = settings.clone();

    // The capture window reads hideImmediately from this.
    let _ = app.emit("settings-changed", &settings);
    Ok(SettingsView {
        settings,
        active_root: state.root.clone(),
    })
}

#[tauri::command]
pub fn get_aliases(state: State<'_, AppState>) -> std::collections::BTreeMap<String, String> {
    state.aliases().into_iter().collect()
}

/// Replace the tag aliases (SPEC 4.5). They steer tags from now on; tags
/// already written into notes are left as they are.
#[tauri::command]
pub async fn set_aliases(
    state: State<'_, AppState>,
    aliases: std::collections::HashMap<String, String>,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    let cleaned = tags::clean_aliases(&aliases)?;
    *state
        .aliases
        .write()
        .map_err(|_| "aliases lock poisoned".to_string())? = cleaned.clone();
    state.persist_index().await?;
    Ok(cleaned.into_iter().collect())
}

#[tauri::command]
pub fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// Put a note back in the queue by hand, for the ones that ended up `failed`
/// (SPEC 5.6).
#[tauri::command]
pub async fn retry_enrichment(
    state: State<'_, AppState>,
    date: String,
    id: String,
) -> Result<(), String> {
    check_date(&date)?;
    enqueue(&state, id, date).await;
    Ok(())
}

#[tauri::command]
pub fn model_status(state: State<'_, AppState>) -> ModelStatus {
    state.model_status()
}

/// Which model is on disk, if any, and what revision it came from.
#[tauri::command]
pub fn installed_model(state: State<'_, AppState>) -> Option<download::InstalledModel> {
    for variant in [Variant::Default, Variant::Light] {
        if let Some(record) = download::installed(&state.root, variant) {
            return Some(record);
        }
    }
    None
}

/// Fetch a model and load it. Capture keeps working throughout; notes simply
/// stay pending until this finishes (SPEC 5.2).
#[tauri::command]
pub async fn download_model(
    app: AppHandle,
    state: State<'_, AppState>,
    variant: Variant,
) -> Result<(), String> {
    let root = state.root.clone();
    let remote: RemoteModel = download::lookup(variant).await?;

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
        set_status(&app, &state, ModelStatus::Absent);
        return Err(e);
    }

    load_model(&app, &state, variant)
}

/// Load a model that is already on disk. Lazy loading on the first job is the
/// normal path (SPEC 5.1); this is the eager one, after a download.
pub fn load_model(
    app: &AppHandle,
    state: &State<'_, AppState>,
    variant: Variant,
) -> Result<(), String> {
    let path = model_file(&state.root, variant);
    let backend = crate::enrich::llama::LlamaCpp::load(&path)?;

    if let Ok(mut slot) = state.backend.write() {
        *slot = Some(std::sync::Arc::new(backend));
    }
    set_status(app, state, ModelStatus::Loaded);
    // Anything that was waiting on a model can go now.
    state.wake.notify_one();
    Ok(())
}

fn set_status(app: &AppHandle, state: &State<'_, AppState>, status: ModelStatus) {
    state.set_model_status(status.clone());
    let _ = app.emit("model-status", &status);
}
