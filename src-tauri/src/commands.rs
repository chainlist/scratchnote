//! Tauri commands, SPEC 8.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use ulid::Ulid;

use crate::enrich::download::{self, RemoteModel};
use crate::enrich::model::{model_file, EmbeddingModel, ModelStatus, Variant};
use crate::enrich::normalize;
use crate::enrich::queue::Job;
use crate::settings::Settings;
use crate::spaces::{self, Space};
use crate::state::AppState;
use crate::storage::daily_file::{self, Note, NotePatch, Status};
use crate::storage::index::{self, IndexEntry};
use crate::storage::{categories, tags};
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
    let space = state.space()?;

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
        .append_note(day_path(&space.root, &date), date, note.clone())
        .await?;

    // A new note only ever adds a line, so the index is appended rather than
    // rewritten. This is the capture path and it has a latency budget.
    let entry = IndexEntry::from(&note);
    let line = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
    space
        .index
        .write()
        .map_err(|_| "index lock poisoned".to_string())?
        .push(entry);
    state
        .writer
        .append_index_line(index::index_path(&space.root), line)
        .await?;
    space.index_changed();

    enqueue(&state, &space, note.id.clone(), note.date.clone()).await;

    let _ = app.emit("note-updated", serde_json::json!({ "id": note.id }));
    Ok(Some(note))
}

/// Queue a note for enrichment and nudge the worker. Already-queued notes are
/// left alone, so saving twice does not enrich twice.
async fn enqueue(state: &State<'_, AppState>, space: &Space, id: String, date: String) {
    let queued = match space.queue.lock() {
        Ok(mut queue) => queue.push(Job::new(id, date)),
        Err(_) => false,
    };
    if !queued {
        return;
    }
    persist_queue(state, space).await;
    state.wake.notify_one();
}

async fn persist_queue(state: &State<'_, AppState>, space: &Space) {
    space.persist_queue(&state.writer).await;
}

/// Read straight from the markdown, because the index deliberately carries no
/// bodies and the day view shows them.
#[tauri::command]
pub async fn get_day(state: State<'_, AppState>, date: String) -> Result<Vec<Note>, String> {
    check_date(&date)?;
    let space = state.space()?;
    let path = day_path(&space.root, &date);
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
    let space = state.space()?;
    let days = space
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
    let space = state.space()?;
    let path = day_path(&space.root, &date);
    if !state.writer.delete_note(path.clone(), id.clone()).await? {
        return Err(format!("no note {id} in {date}"));
    }

    // A delete rewrites the day file, so the day's entries are reparsed and
    // the whole index written back (SPEC 4.4).
    let entries = index::parse_day(&path, &date);
    space
        .index
        .write()
        .map_err(|_| "index lock poisoned".to_string())?
        .replace_day(&date, entries);
    space.persist_index(&state.writer).await?;

    if let Ok(mut queue) = space.queue.lock() {
        queue.remove(&id);
    }
    persist_queue(&state, &space).await;

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
    let space = state.space()?;
    let body = body.trim().to_string();
    if body.is_empty() {
        return Err("a note cannot be empty; delete it instead".to_string());
    }
    let current = read_note(&space, &date, &id).await?;
    if current.body == body {
        return Ok(current);
    }

    let path = day_path(&space.root, &date);
    if !state.writer.replace_body(path, id.clone(), body).await? {
        return Err(format!("no note {id} in {date}"));
    }
    reindex_day(&state, &space, &date).await?;
    if current.status != Status::Manual {
        enqueue(&state, &space, id.clone(), date.clone()).await;
    }

    let _ = app.emit("note-updated", serde_json::json!({ "id": id }));
    read_note(&space, &date, &id).await
}

/// Set a note's subject, category and tags by hand. The note becomes
/// `manual`, which enrichment never overwrites (SPEC 4.2). `None` keeps the
/// current value. `tags` are the tags besides the category, and an empty
/// `category` takes the note out of every category.
#[tauri::command]
pub async fn update_note_meta(
    app: AppHandle,
    state: State<'_, AppState>,
    date: String,
    id: String,
    subject: Option<String>,
    category: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<Note, String> {
    check_date(&date)?;
    let space = state.space()?;
    let current = read_note(&space, &date, &id).await?;
    let vocabulary = space.vocabulary();
    let (current_category, current_tags) = categories::split(&vocabulary.categories, &current.tags);

    let subject = match subject {
        // The subject is the note's heading, so it has to stay on one line.
        Some(raw) => {
            Some(raw.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|s| !s.is_empty())
        }
        None => current.subject.clone(),
    };
    let category = match category {
        Some(raw) => normalize::manual(&[raw], &vocabulary).into_iter().next(),
        None => current_category,
    };
    let rest = match tags {
        Some(raw) => normalize::manual(&raw, &vocabulary),
        None => current_tags,
    };
    let tags = categories::join(category.as_deref(), &rest);
    // The file keeps a summary and its tags as one two-line block, and that
    // block cannot be written with the tag line empty.
    if tags.is_empty() && current.summary.is_some() {
        return Err("keep a category or at least one tag".to_string());
    }

    // A category typed in by hand joins the list, as one the model invents
    // does, so the sidebar shows it and later notes are offered it.
    if let Some(contents) = category
        .as_deref()
        .and_then(|c| categories::with_added(&space.root, c))
    {
        state
            .writer
            .write_index(categories::categories_path(&space.root), contents)
            .await?;
    }

    let patch = NotePatch {
        subject,
        summary: current.summary.clone(),
        tags,
        status: Status::Manual,
    };
    let path = day_path(&space.root, &date);
    if !state.writer.update_note(path, id.clone(), patch).await? {
        return Err(format!("no note {id} in {date}"));
    }
    reindex_day(&state, &space, &date).await?;

    // Nothing left for the model to do with it.
    if let Ok(mut queue) = space.queue.lock() {
        queue.remove(&id);
    }
    persist_queue(&state, &space).await;

    let _ = app.emit("note-updated", serde_json::json!({ "id": id }));
    read_note(&space, &date, &id).await
}

async fn read_note(space: &Space, date: &str, id: &str) -> Result<Note, String> {
    let contents = tokio::fs::read_to_string(day_path(&space.root, date))
        .await
        .map_err(|e| e.to_string())?;
    daily_file::parse_notes(&contents, date, &relative_day_path(date))
        .into_iter()
        .find(|note| note.id == id)
        .ok_or_else(|| format!("no note {id} in {date}"))
}

/// After rewriting a day file: reparse the day and write the index back.
async fn reindex_day(state: &State<'_, AppState>, space: &Space, date: &str) -> Result<(), String> {
    let entries = index::parse_day(&day_path(&space.root, date), date);
    space
        .index
        .write()
        .map_err(|_| "index lock poisoned".to_string())?
        .replace_day(date, entries);
    space.persist_index(&state.writer).await
}

/// Reparse every markdown file and replace the cache. Returns how many notes
/// the rebuilt index holds.
#[tauri::command]
pub async fn rebuild_index(app: AppHandle, state: State<'_, AppState>) -> Result<usize, String> {
    let space = state.space()?;
    let root = space.root.clone();
    let rebuilt = tauri::async_runtime::spawn_blocking(move || index::rebuild(&root))
        .await
        .map_err(|e| e.to_string())?;

    let count = rebuilt.len();
    *space
        .index
        .write()
        .map_err(|_| "index lock poisoned".to_string())? = rebuilt;
    // Aliases are hand-edited in tags.json, so a rebuild is also when edits
    // made there are picked up.
    if let Ok(mut aliases) = space.aliases.write() {
        *aliases = tags::load_aliases(&space.root);
    }
    space.persist_index(&state.writer).await?;

    let _ = app.emit("index-rebuilt", ());
    Ok(count)
}

/// Rebuild the index, then hand every note back to the model for a fresh
/// subject, summary and tags. Every note is cleared to pending first, hand
/// edits included, so the tag list empties and the model starts from no
/// vocabulary but the aliases. Returns how many notes were queued.
#[tauri::command]
pub async fn regenerate_all(app: AppHandle, state: State<'_, AppState>) -> Result<usize, String> {
    rebuild_index(app.clone(), state.clone()).await?;
    let space = state.space()?;

    let notes: Vec<(String, String)> = space
        .index
        .read()
        .map_err(|_| "index lock poisoned".to_string())?
        .entries()
        .map(|e| (e.id.clone(), e.date.clone()))
        .collect();

    let cleared = NotePatch {
        subject: None,
        summary: None,
        tags: Vec::new(),
        status: Status::Pending,
    };
    let mut touched_days = std::collections::BTreeSet::new();
    for (id, date) in &notes {
        state
            .writer
            .update_note(day_path(&space.root, date), id.clone(), cleared.clone())
            .await?;
        touched_days.insert(date.clone());
    }
    // One index write for the lot, not one per day.
    {
        let mut idx = space
            .index
            .write()
            .map_err(|_| "index lock poisoned".to_string())?;
        for date in &touched_days {
            idx.replace_day(date, index::parse_day(&day_path(&space.root, date), date));
        }
    }
    space.persist_index(&state.writer).await?;

    if let Ok(mut queue) = space.queue.lock() {
        for (id, date) in &notes {
            queue.push(Job::new(id.clone(), date.clone()));
        }
    }
    persist_queue(&state, &space).await;
    state.wake.notify_one();

    let _ = app.emit("index-rebuilt", ());
    Ok(notes.len())
}

/// Every tag in use with how many notes carry it, most used first.
#[tauri::command]
pub fn list_tags(state: State<'_, AppState>) -> Result<Vec<(String, u32)>, String> {
    let space = state.space()?;
    let counts = space
        .index
        .read()
        .map_err(|_| "index lock poisoned".to_string())?
        .tag_counts();
    Ok(tags::by_count(&counts))
}

/// The categories notes are filed under, with how many carry each, most used
/// first. Categories no note carries yet are left out.
#[tauri::command]
pub fn list_categories(state: State<'_, AppState>) -> Result<Vec<(String, u32)>, String> {
    let space = state.space()?;
    let counts = space
        .index
        .read()
        .map_err(|_| "index lock poisoned".to_string())?
        .tag_counts();
    Ok(categories::in_use(&categories::load(&space.root), &counts))
}

/// Every category on the list, in file order, used or not, for the note
/// editor to offer.
#[tauri::command]
pub fn category_names(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(categories::load(&state.space()?.root))
}

/// Words and `#tag` filters across every day (SPEC 6).
#[tauri::command]
pub fn search(state: State<'_, AppState>, query: String) -> Result<Vec<Note>, String> {
    let space = state.space()?;
    let aliases = space.aliases();
    let idx = space
        .index
        .read()
        .map_err(|_| "index lock poisoned".to_string())?;
    Ok(crate::search::search(&idx, &query, &aliases))
}

/// What a chat sends the page while it replies.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ChatEvent {
    /// The index the model reads, numbered as it sees it: note `n` is
    /// `notes[n - 1]`. First, so citations resolve while the reply streams.
    Notes { notes: Vec<IndexEntry> },
    /// The next piece of the reply.
    Token { text: String },
    /// The reply is complete, or was stopped; these are the notes it cites.
    Done { cited: Vec<usize> },
}

const NO_MODEL: &str =
    "Chatting needs a model. Download one in Settings, or wait for the download to finish.";
const MODEL_OFF: &str = "Chatting needs the model, which is turned off in Settings.";

/// The loaded model, or why chat cannot have one.
fn chat_backend(
    app: &AppHandle,
) -> Result<std::sync::Arc<dyn crate::enrich::model::Backend>, String> {
    if !app.state::<AppState>().model_enabled() {
        return Err(MODEL_OFF.to_string());
    }
    crate::enrich::worker::backend(app).ok_or_else(|| NO_MODEL.to_string())
}

/// The notes closest to the last message, as positions in `notes` with their
/// bodies filled in, for `chat::reply`. Empty without the embedding model or
/// when embedding fails, and the chat then reads the index alone.
fn retrieve(
    app: &AppHandle,
    space: &Space,
    notes: &mut [IndexEntry],
    messages: &[crate::chat::Message],
) -> Vec<usize> {
    let (Some(embedder), Some(last)) = (crate::embed::embedder(app), messages.last()) else {
        return Vec::new();
    };
    let query = match embedder.embed_query(last.content.trim()) {
        Ok(query) => query,
        Err(e) => {
            log::warn!("could not embed the question, answering from the index: {e}");
            return Vec::new();
        }
    };
    let hits = space.nearest(&query, crate::chat::MAX_RETRIEVED);
    match space.index.read() {
        Ok(index) => crate::chat::retrieved(notes, &hits, &index),
        Err(_) => Vec::new(),
    }
}

/// Reply to the last message of a conversation about the open space's
/// notes, streamed through `on_event`. The model reads that space's
/// `index.jsonl`, and with the embedding model the text of the few notes
/// closest to the last message, taken from the index in memory. It never
/// reads the markdown. A new message, or `stop_chat`, ends the reply being
/// written.
#[tauri::command]
pub async fn chat(
    app: AppHandle,
    state: State<'_, AppState>,
    messages: Vec<crate::chat::Message>,
    on_event: tauri::ipc::Channel<ChatEvent>,
) -> Result<(), String> {
    let run = crate::chat::begin();
    let space = state.space()?;
    let path = index::index_path(&space.root);

    // Reading the file, loading the models and running them all block, so
    // none of it may run on the async runtime.
    tauri::async_runtime::spawn_blocking(move || {
        let mut notes = crate::chat::read_index(&path)?;
        let _ = on_event.send(ChatEvent::Notes {
            notes: notes.clone(),
        });
        let backend = chat_backend(&app)?;
        let state = app.state::<AppState>();
        let started = std::time::Instant::now();
        let retrieved = retrieve(&app, &space, &mut notes, &messages);
        let retrieval = started.elapsed();
        let reply = crate::chat::reply(
            &space.name,
            &notes,
            &retrieved,
            &messages,
            Local::now().date_naive(),
            backend.as_ref(),
            &mut |piece| {
                // Keeps the idle unload away for as long as the reply runs.
                state.mark_used();
                let _ = on_event.send(ChatEvent::Token {
                    text: piece.to_string(),
                });
                crate::chat::is_current(run)
            },
        )?;
        log::info!(
            "chat replied over {} notes in {:.1}s, {} retrieved in {:.2}s",
            notes.len(),
            started.elapsed().as_secs_f32(),
            retrieved.len(),
            retrieval.as_secs_f32()
        );
        let _ = on_event.send(ChatEvent::Done {
            cited: crate::chat::cited(&reply, notes.len()),
        });
        Ok(())
    })
    .await
    .map_err(|e| format!("the chat failed: {e}"))?
}

/// End the reply being written, if any. It ends as a normal reply would,
/// with what was written so far.
#[tauri::command]
pub fn stop_chat() {
    crate::chat::stop();
}

/// Read the open space's index into the model's cache ahead of the first
/// message, loading the model if need be, so the first reply starts sooner.
/// The page calls it when the chat opens.
#[tauri::command]
pub async fn warm_chat(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let space = state.space()?;
    let (name, path) = (space.name.clone(), index::index_path(&space.root));
    tauri::async_runtime::spawn_blocking(move || {
        let notes = crate::chat::read_index(&path)?;
        let backend = chat_backend(&app)?;
        app.state::<AppState>().mark_used();
        let started = std::time::Instant::now();
        let prefix = crate::chat::prefix(&name, &notes, Local::now().date_naive());
        backend.prefill_long(&prefix)?;
        log::info!(
            "chat read {} notes ahead in {:.1}s",
            notes.len(),
            started.elapsed().as_secs_f32()
        );
        Ok(())
    })
    .await
    .map_err(|e| format!("warming the chat failed: {e}"))?
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
    Ok(SettingsView {
        settings: current_settings(&state)?,
        active_root: state.root.clone(),
    })
}

/// Save settings (SPEC 7). The hotkey and the model switch straight away; a
/// new notes root is saved but only used from the next launch.
#[tauri::command]
pub async fn set_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<SettingsView, String> {
    settings.validate()?;
    let old = current_settings(&state)?;

    if settings.capture_hotkey != old.capture_hotkey {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        let shortcuts = app.global_shortcut();
        // Bind the new one first, so a key the OS refuses leaves the old one
        // working instead of leaving no hotkey at all.
        shortcuts
            .register(settings.capture_hotkey.as_str())
            .map_err(|e| format!("cannot use {} as the hotkey: {e}", settings.capture_hotkey))?;
        let _ = shortcuts.unregister(old.capture_hotkey.as_str());
    }

    // A different model, or the same one on different hardware, takes a
    // reload either way.
    let model_changed = settings.model_enabled != old.model_enabled
        || settings.model_variant != old.model_variant
        || settings.model_path != old.model_path
        || settings.use_gpu != old.use_gpu;
    save_settings(&app, &state, settings.clone()).await?;
    if model_changed {
        unload_model(&app, &state);
        state.wake.notify_one();
        state.embed_wake.notify_one();
    }

    Ok(SettingsView {
        settings,
        active_root: state.root.clone(),
    })
}

fn current_settings(state: &State<'_, AppState>) -> Result<Settings, String> {
    Ok(state
        .settings
        .read()
        .map_err(|_| "settings lock poisoned".to_string())?
        .clone())
}

async fn save_settings(
    app: &AppHandle,
    state: &State<'_, AppState>,
    settings: Settings,
) -> Result<(), String> {
    state
        .writer
        .write_index(crate::settings::file(app), settings.to_json())
        .await?;
    *state
        .settings
        .write()
        .map_err(|_| "settings lock poisoned".to_string())? = settings.clone();
    // The capture window reads hideImmediately from this.
    let _ = app.emit("settings-changed", &settings);
    Ok(())
}

#[tauri::command]
pub fn get_aliases(state: State<'_, AppState>) -> std::collections::BTreeMap<String, String> {
    state
        .space()
        .map(|space| space.aliases().into_iter().collect())
        .unwrap_or_default()
}

/// Replace the tag aliases (SPEC 4.5). They steer tags from now on; tags
/// already written into notes are left as they are.
#[tauri::command]
pub async fn set_aliases(
    state: State<'_, AppState>,
    aliases: std::collections::HashMap<String, String>,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    let cleaned = tags::clean_aliases(&aliases)?;
    let space = state.space()?;
    *space
        .aliases
        .write()
        .map_err(|_| "aliases lock poisoned".to_string())? = cleaned.clone();
    space.persist_index(&state.writer).await?;
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
    let space = state.space()?;
    enqueue(&state, &space, id, date).await;
    Ok(())
}

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
pub async fn download_embedding_model(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
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

/// Drop the loaded model. The next job loads whatever the settings now point
/// at, so this is also how a model switch takes effect.
fn unload_model(app: &AppHandle, state: &State<'_, AppState>) {
    let status = state.unload_model();
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
    // go of it, and give a job still holding it a moment to finish.
    state.swapping.store(true, Ordering::SeqCst);
    unload_model(&app, &state);
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

/// One row of the space switcher.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceSummary {
    pub name: String,
    pub notes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpacesView {
    pub active: String,
    pub spaces: Vec<SpaceSummary>,
}

fn spaces_view(state: &AppState) -> Result<SpacesView, String> {
    let active = state.space()?.name.clone();
    let spaces = state
        .spaces
        .read()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .iter()
        .map(|s| SpaceSummary {
            name: s.name.clone(),
            notes: s.note_count(),
        })
        .collect();
    Ok(SpacesView { active, spaces })
}

/// Load a space and start watching it. Used at startup and whenever a space
/// is made or renamed.
pub fn open_space(app: &AppHandle, name: &str, root: PathBuf) -> Arc<Space> {
    let embed_wake = app.state::<AppState>().embed_wake.clone();
    let (space, refresh) = Space::open(name, root, embed_wake);
    let space = Arc::new(space);
    if let Err(e) = crate::watcher::start(app.clone(), &space) {
        log::error!("could not watch the notes of {name}: {e}");
    }
    if refresh {
        let app = app.clone();
        let space = space.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            if let Err(e) = space.persist_index(&state.writer).await {
                log::warn!("could not write the index of {} back: {e}", space.name);
            }
        });
    }
    space
}

/// Record the registry and tell both windows the spaces changed.
async fn spaces_changed(app: &AppHandle, state: &AppState) -> Result<SpacesView, String> {
    let json = state
        .registry
        .read()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .to_json();
    state
        .writer
        .write_index(spaces::registry_path(&state.root), json)
        .await?;
    let view = spaces_view(state)?;
    let _ = app.emit("spaces-changed", &view);
    Ok(view)
}

/// A name no other space has, compared the way Windows compares folder names.
fn unused_name(state: &AppState, raw: &str, renaming: Option<&str>) -> Result<String, String> {
    let name = spaces::check_name(raw)?;
    let taken = state
        .spaces
        .read()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .iter()
        .any(|s| Some(s.name.as_str()) != renaming && s.name.to_lowercase() == name.to_lowercase());
    if taken {
        return Err(format!("there is already a space called {name}"));
    }
    Ok(name)
}

/// Swap a space for its replacement, keeping them in name order.
fn replace_space(state: &AppState, old: &Arc<Space>, new: Arc<Space>) -> Result<(), String> {
    let mut spaces = state
        .spaces
        .write()
        .map_err(|_| "spaces lock poisoned".to_string())?;
    if let Some(slot) = spaces.iter_mut().find(|s| Arc::ptr_eq(s, old)) {
        *slot = new;
    }
    sort_spaces(&mut spaces);
    Ok(())
}

pub fn sort_spaces(spaces: &mut [Arc<Space>]) {
    spaces.sort_by_key(|s| s.name.to_lowercase());
}

#[tauri::command]
pub fn list_spaces(state: State<'_, AppState>) -> Result<SpacesView, String> {
    spaces_view(&state)
}

/// Make a space and open it. Its folder is `spaces/<name>/` under the root.
#[tauri::command]
pub async fn create_space(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<SpacesView, String> {
    let name = unused_name(&state, &name, None)?;
    let root = spaces::spaces_dir(&state.root).join(&name);
    if root.exists() {
        return Err(format!("{} is already there", root.display()));
    }
    tokio::fs::create_dir_all(root.join("notes"))
        .await
        .map_err(|e| format!("could not create {}: {e}", root.display()))?;

    let space = open_space(&app, &name, root);
    {
        let mut spaces = state
            .spaces
            .write()
            .map_err(|_| "spaces lock poisoned".to_string())?;
        spaces.push(space);
        sort_spaces(&mut spaces);
    }
    state
        .registry
        .write()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .active = name;
    spaces_changed(&app, &state).await
}

/// Open another space. Every note command acts on it from then on, and the
/// capture window saves into it.
#[tauri::command]
pub async fn set_active_space(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<SpacesView, String> {
    if state.find_space(&name).is_none() {
        return Err(format!("no space {name}"));
    }
    state
        .registry
        .write()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .active = name;
    // Its notes are now first in line for the model.
    state.wake.notify_one();
    spaces_changed(&app, &state).await
}

/// Rename a space, and with it its folder.
#[tauri::command]
pub async fn rename_space(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    new_name: String,
) -> Result<SpacesView, String> {
    let old = state
        .find_space(&name)
        .ok_or_else(|| format!("no space {name}"))?;
    let new_name = unused_name(&state, &new_name, Some(&name))?;
    if new_name == name {
        return spaces_view(&state);
    }

    // The watcher holds the folder open, and Windows will not move an open
    // folder, so the old space lets go of it first.
    old.retire();
    let to = spaces::spaces_dir(&state.root).join(&new_name);
    if let Err(e) = tokio::fs::rename(&old.root, &to).await {
        let back = open_space(&app, &name, old.root.clone());
        replace_space(&state, &old, back)?;
        return Err(format!("could not rename {}: {e}", old.root.display()));
    }
    let renamed = open_space(&app, &new_name, to);
    replace_space(&state, &old, renamed)?;

    {
        let mut registry = state
            .registry
            .write()
            .map_err(|_| "spaces lock poisoned".to_string())?;
        if registry.active == name {
            registry.active = new_name;
        }
    }
    // A job the old space had in hand was dropped; its note is still pending
    // and was queued again by the reopened space.
    state.wake.notify_one();
    spaces_changed(&app, &state).await
}

/// Take a space out of the app. Its folder is moved to `.scratchnote/trash/`
/// rather than deleted, so its notes can be recovered by moving the folder
/// back under `spaces/`.
#[tauri::command]
pub async fn delete_space(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<SpacesView, String> {
    let space = state
        .find_space(&name)
        .ok_or_else(|| format!("no space {name}"))?;
    let last = state
        .spaces
        .read()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .len()
        <= 1;
    if last {
        return Err("the last space cannot be deleted".to_string());
    }

    space.retire();
    let trash = state.root.join(".scratchnote").join("trash");
    let to = trash.join(format!("{name} {}", Local::now().format("%Y-%m-%d %H%M%S")));
    let moved = match tokio::fs::create_dir_all(&trash).await {
        Ok(()) => tokio::fs::rename(&space.root, &to).await,
        Err(e) => Err(e),
    };
    if let Err(e) = moved {
        let back = open_space(&app, &name, space.root.clone());
        replace_space(&state, &space, back)?;
        return Err(format!(
            "could not move {} to the trash: {e}",
            space.root.display()
        ));
    }

    state
        .spaces
        .write()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .retain(|s| !Arc::ptr_eq(s, &space));
    let first = state
        .spaces
        .read()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .first()
        .map(|s| s.name.clone());
    {
        let mut registry = state
            .registry
            .write()
            .map_err(|_| "spaces lock poisoned".to_string())?;
        if let (true, Some(first)) = (registry.active == name, first) {
            registry.active = first;
        }
    }
    spaces_changed(&app, &state).await
}

/// Show a space's folder in the system file manager. The path is looked up
/// here rather than passed in, so the page can only open folders that are
/// spaces.
#[tauri::command]
pub fn open_space_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let space = state
        .find_space(&name)
        .ok_or_else(|| format!("no space {name}"))?;
    app.opener()
        .open_path(space.root.to_string_lossy(), None::<&str>)
        .map_err(|e| format!("could not open {}: {e}", space.root.display()))
}
