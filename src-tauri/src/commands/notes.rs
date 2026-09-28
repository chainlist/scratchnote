//! Notes: capture, the day view, edits, and the index behind them.

use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use ulid::Ulid;

use crate::enrich::normalize;
use crate::enrich::queue::Job;
use crate::spaces::Space;
use crate::state::AppState;
use crate::storage::categories;
use crate::storage::daily_file::{self, Kind, Note, NotePatch, Status};
use crate::storage::index::{self, IndexEntry};
use crate::storage::{check_date, day_path, relative_day_path};

#[derive(Debug, Serialize)]
pub struct DaySummary {
    pub date: String,
    pub count: usize,
}

/// Append a note to a day's file, today's unless another date is given, and
/// tell the rest of the app about it. The note carries the current time
/// either way. An empty body is a no-op, not an error.
#[tauri::command]
pub async fn save_note(
    app: AppHandle,
    state: State<'_, AppState>,
    body: String,
    date: Option<String>,
) -> Result<Option<Note>, String> {
    let body = body.trim().to_string();
    if body.is_empty() {
        return Ok(None);
    }
    if let Some(date) = &date {
        check_date(date)?;
    }
    let space = state.space()?;

    let now = Local::now();
    let date = date.unwrap_or_else(|| now.format("%Y-%m-%d").to_string());
    let note = Note {
        id: Ulid::generate().to_string(),
        time: now.format("%H:%M").to_string(),
        file: relative_day_path(&date),
        subject: None,
        category: None,
        status: Status::Pending,
        hash: daily_file::body_hash(&body),
        lang: None,
        body,
        date: date.clone(),
        kind: Kind::Note,
        missing: false,
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
pub(crate) async fn enqueue(state: &State<'_, AppState>, space: &Space, id: String, date: String) {
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

pub(crate) async fn persist_queue(state: &State<'_, AppState>, space: &Space) {
    space.persist_queue(&state.writer).await;
}

/// Read straight from the markdown, because the index deliberately carries no
/// bodies and the day view shows them. The day's pages come from the index,
/// which holds their files' text, and a stub whose page is gone comes back
/// as a missing page (SPEC 3.5).
#[tauri::command]
pub async fn get_day(state: State<'_, AppState>, date: String) -> Result<Vec<Note>, String> {
    check_date(&date)?;
    let space = state.space()?;
    let path = day_path(&space.root, &date);
    let contents = match tokio::fs::read_to_string(&path).await {
        Ok(contents) => contents,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.to_string()),
    };
    let mut notes = daily_file::parse_notes(&contents, &date, &relative_day_path(&date));
    {
        let idx = space
            .index
            .read()
            .map_err(|_| "index lock poisoned".to_string())?;
        notes.extend(idx.pages_on(&date).map(IndexEntry::to_note));
        for stub in daily_file::parse_stubs(&contents) {
            if idx.page(&stub.id).is_none() {
                notes.push(missing_page(&date, stub));
            }
        }
    }
    notes.sort_by(|a, b| a.time.cmp(&b.time));
    Ok(notes)
}

/// What the day view shows for a stub whose page file is gone.
fn missing_page(date: &str, stub: daily_file::Stub) -> Note {
    Note {
        id: stub.id,
        date: date.to_string(),
        time: stub.time,
        file: stub.target,
        subject: Some(stub.title).filter(|t| !t.is_empty()),
        category: None,
        status: Status::Done,
        hash: String::new(),
        lang: None,
        body: String::new(),
        kind: Kind::Page,
        missing: true,
    }
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
/// unless the user has taken the note over (SPEC 4.2) or only ticked task
/// boxes (SPEC 3.4).
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

    let requeue =
        current.status != Status::Manual && !daily_file::only_ticks_changed(&current.body, &body);
    let path = day_path(&space.root, &date);
    if !state.writer.replace_body(path, id.clone(), body).await? {
        return Err(format!("no note {id} in {date}"));
    }
    if requeue {
        mark_pending(&state, &space, &current).await?;
    }
    reindex_day(&state, &space, &date).await?;
    if requeue {
        enqueue(&state, &space, id.clone(), date.clone()).await;
    }

    let _ = app.emit("note-updated", serde_json::json!({ "id": id }));
    read_note(&space, &date, &id).await
}

/// Set a note's subject and category by hand. The note becomes `manual`,
/// which enrichment never overwrites (SPEC 4.2). `None` keeps the current
/// value, and an empty `category` takes the note out of every category.
#[tauri::command]
pub async fn update_note_meta(
    app: AppHandle,
    state: State<'_, AppState>,
    date: String,
    id: String,
    subject: Option<String>,
    category: Option<String>,
) -> Result<Note, String> {
    check_date(&date)?;
    let space = state.space()?;
    let current = read_note(&space, &date, &id).await?;

    let subject = match subject {
        // The subject is the note's heading, so it has to stay on one line.
        Some(raw) => {
            Some(raw.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|s| !s.is_empty())
        }
        None => current.subject.clone(),
    };
    let category = match category {
        Some(raw) => normalize::clean(&raw),
        None => current.category.clone(),
    };

    // A category typed in by hand joins the list, so the model is offered it
    // for later notes.
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
        category,
        status: Status::Manual,
        lang: current.lang.clone(),
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

fn is_page(space: &Space, id: &str) -> bool {
    space.index.read().is_ok_and(|idx| idx.page(id).is_some())
}

pub(crate) async fn read_note(space: &Space, date: &str, id: &str) -> Result<Note, String> {
    let contents = tokio::fs::read_to_string(day_path(&space.root, date))
        .await
        .map_err(|e| e.to_string())?;
    daily_file::parse_notes(&contents, date, &relative_day_path(date))
        .into_iter()
        .find(|note| note.id == id)
        .ok_or_else(|| format!("no note {id} in {date}"))
}

/// Set a note back to `pending` as it returns to the queue, so the day view
/// shows it waiting. Its labels stay until the model replaces them.
async fn mark_pending(
    state: &State<'_, AppState>,
    space: &Space,
    note: &Note,
) -> Result<(), String> {
    let patch = NotePatch {
        subject: note.subject.clone(),
        category: note.category.clone(),
        status: Status::Pending,
        lang: note.lang.clone(),
    };
    let path = day_path(&space.root, &note.date);
    state
        .writer
        .update_note(path, note.id.clone(), patch)
        .await?;
    Ok(())
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
    space.persist_index(&state.writer).await?;

    let _ = app.emit("index-rebuilt", ());
    Ok(count)
}

/// Rebuild the index, then hand every note back to the model for a fresh
/// subject and category. Every note is cleared to pending first, hand edits
/// included. Returns how many notes were queued.
#[tauri::command]
pub async fn regenerate_all(app: AppHandle, state: State<'_, AppState>) -> Result<usize, String> {
    rebuild_index(app.clone(), state.clone()).await?;
    let space = state.space()?;

    let (notes, pages): (Vec<IndexEntry>, Vec<IndexEntry>) = space
        .index
        .read()
        .map_err(|_| "index lock poisoned".to_string())?
        .entries()
        .cloned()
        .partition(|e| e.kind == Kind::Note);

    let cleared = NotePatch {
        subject: None,
        category: None,
        status: Status::Pending,
        lang: None,
    };
    let mut touched_days = std::collections::BTreeSet::new();
    for note in &notes {
        state
            .writer
            .update_note(
                day_path(&space.root, &note.date),
                note.id.clone(),
                cleared.clone(),
            )
            .await?;
        touched_days.insert(note.date.clone());
    }
    // A page keeps its title, which is the user's; only its category goes.
    for page in &pages {
        let file = page.file.clone();
        state
            .writer
            .rewrite(space.root.join(&page.file), move |existing| {
                crate::storage::page_file::update_meta(
                    existing?,
                    &file,
                    None,
                    Status::Pending,
                    None,
                )
            })
            .await?;
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
        for page in &pages {
            if let Some(entry) = index::parse_page(&space.root, &space.root.join(&page.file)) {
                idx.replace_page(entry);
            }
        }
    }
    let notes: Vec<(String, String)> = notes
        .iter()
        .chain(&pages)
        .map(|e| (e.id.clone(), e.date.clone()))
        .collect();
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

#[tauri::command]
pub fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// Put a note back in the queue by hand, for the ones that ended up `failed`
/// (SPEC 5.6).
#[tauri::command]
pub async fn retry_enrichment(
    app: AppHandle,
    state: State<'_, AppState>,
    date: String,
    id: String,
) -> Result<(), String> {
    check_date(&date)?;
    let space = state.space()?;
    if is_page(&space, &id) {
        return crate::pages::retry(&app, &state, &space, &id).await;
    }
    let note = read_note(&space, &date, &id).await?;
    // A manual note is never re-enriched, so it must not be unlocked here.
    if note.status == Status::Manual {
        return Ok(());
    }
    mark_pending(&state, &space, &note).await?;
    reindex_day(&state, &space, &date).await?;
    enqueue(&state, &space, id.clone(), date).await;
    let _ = app.emit("note-updated", serde_json::json!({ "id": id }));
    Ok(())
}

/// Open a link from a note in the default browser or mail client. Only web
/// and mail links go through, so a note cannot launch files or programs.
#[tauri::command]
pub fn open_link(app: AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let lower = url.to_ascii_lowercase();
    if !["http://", "https://", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
    {
        return Err(format!("not a web or mail link: {url}"));
    }
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| format!("could not open {url}: {e}"))
}
