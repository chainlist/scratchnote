//! Notes: capture, the day view, edits, and the index behind them.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use ulid::Ulid;

use super::blocking;
use crate::spaces::Space;
use crate::state::{read_lock, AppState};
use crate::storage::daily_file::{self, Kind, Note};
use crate::storage::index::{self, IndexEntry};
use crate::storage::{check_date, day_path, page_file, relative_day_path};
use crate::Result;

#[derive(Debug, Serialize)]
pub struct DaySummary {
    pub date: String,
    pub count: usize,
    /// How many words the day's notes hold, its pages left out.
    pub words: usize,
}

/// Append a note to a day's file, today's unless another date is given, and
/// tell the rest of the app about it. The note carries the current time
/// either way. An empty body is a no-op, not an error.
///
/// It goes into the open space unless another is named, as the capture
/// window may (SPEC 3.1). A space not open takes it as an edit from outside:
/// its index reads the day again when it opens.
#[tauri::command]
pub async fn save_note(
    app: AppHandle,
    state: State<'_, AppState>,
    body: String,
    date: Option<String>,
    space: Option<String>,
) -> Result<Option<Note>> {
    let body = body.trim().to_string();
    if body.is_empty() {
        return Ok(None);
    }
    if let Some(date) = &date {
        check_date(date)?;
    }
    let space = state.space_or_open(space.as_deref())?;

    let now = Local::now();
    let date = date.unwrap_or_else(|| now.format("%Y-%m-%d").to_string());
    let note = Note {
        id: Ulid::generate().to_string(),
        time: now.format("%H:%M").to_string(),
        file: relative_day_path(&date),
        subject: None,
        hash: daily_file::body_hash(&body),
        on: crate::ahead::day_ahead(&body, &date),
        ahead_off: false,
        body,
        date: date.clone(),
        kind: Kind::Note,
        missing: false,
    };

    state
        .writer
        .append_note(day_path(&space.root, &date), date, note.clone())
        .await?;

    space.note_added(&state.writer, &note).await?;

    let _ = app.emit("note-updated", serde_json::json!({ "id": note.id }));
    Ok(Some(note))
}

/// Read straight from the markdown, because the index deliberately carries no
/// bodies and the day view shows them. The day's pages come from the index,
/// with their text from search.db, and a stub whose page is gone comes back
/// as a missing page (SPEC 3.5).
#[tauri::command]
pub async fn get_day(state: State<'_, AppState>, date: String) -> Result<Vec<Note>> {
    check_date(&date)?;
    let space = state.space()?;
    let path = day_path(&space.root, &date);
    let contents = match tokio::fs::read_to_string(&path).await {
        Ok(contents) => contents,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    let mut notes = daily_file::parse_notes(&contents, &date, &relative_day_path(&date));
    let mut pages: Vec<Note> = {
        let idx = read_lock(&space.index, "index")?;
        for stub in daily_file::parse_stubs(&contents) {
            if idx.page(&stub.id).is_none() {
                notes.push(missing_page(&date, stub));
            }
        }
        idx.pages_on(&date).map(IndexEntry::to_note).collect()
    };
    space.fill_bodies(&mut pages);
    notes.extend(pages);
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
        hash: String::new(),
        on: None,
        ahead_off: false,
        body: String::new(),
        kind: Kind::Page,
        missing: true,
    }
}

#[tauri::command]
pub async fn list_days(state: State<'_, AppState>) -> Result<Vec<DaySummary>> {
    let space = state.space()?;
    let index = read_lock(&space.index, "index")?;
    Ok(index
        .days()
        .into_iter()
        .map(|(date, count)| DaySummary {
            words: index.words_on(&date),
            date,
            count,
        })
        .collect())
}

/// One note or page of the open space with its text, `None` once it is
/// gone, as the map shows a note pointed at (SPEC 6.5).
#[tauri::command]
pub async fn get_note(state: State<'_, AppState>, id: String) -> Result<Option<Note>> {
    let space = state.space()?;
    let note = space.read(|idx, db| match idx.entries().find(|entry| entry.id == id) {
        Some(entry) => Ok(crate::search::with_bodies(db, [entry])?.pop()),
        None => Ok(None),
    })?;
    Ok(note.flatten())
}

/// Some notes or pages of the open space with their text, in the order
/// asked, leaving out those gone, as the Threads page shows the first notes
/// of the threads it draws (SPEC 6.4).
#[tauri::command]
pub async fn get_notes(state: State<'_, AppState>, ids: Vec<String>) -> Result<Vec<Note>> {
    let space = state.space()?;
    let notes = space.read(|idx, db| {
        let entries: HashMap<&str, &IndexEntry> =
            idx.entries().map(|e| (e.id.as_str(), e)).collect();
        let asked = ids
            .iter()
            .filter_map(|id| entries.get(id.as_str()).copied());
        crate::search::with_bodies(db, asked)
    })?;
    Ok(notes.unwrap_or_default())
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
) -> Result<()> {
    check_date(&date)?;
    let space = state.space()?;
    let path = day_path(&space.root, &date);
    if !state.writer.delete_note(path.clone(), id.clone()).await? {
        return Err(format!("no note {id} in {date}").into());
    }

    // A delete rewrites the day file, so the day's entries are reparsed and
    // the whole index written back (SPEC 4.4).
    space.day_changed(&date)?;
    space.persist_index(&state.writer).await?;

    let _ = app.emit("note-updated", serde_json::json!({ "id": id }));
    Ok(())
}

/// Edit a note's body in the app.
#[tauri::command]
pub async fn update_note(
    app: AppHandle,
    state: State<'_, AppState>,
    date: String,
    id: String,
    body: String,
) -> Result<Note> {
    check_date(&date)?;
    let space = state.space()?;
    let body = body.trim().to_string();
    if body.is_empty() {
        return Err("a note cannot be empty; delete it instead".into());
    }
    let current = read_note(&space, &date, &id).await?;
    if current.body == body {
        return Ok(current);
    }

    let path = day_path(&space.root, &date);
    if !state.writer.replace_body(path, id.clone(), body).await? {
        return Err(format!("no note {id} in {date}").into());
    }
    reindex_day(&state, &space, &date).await?;

    let _ = app.emit("note-updated", serde_json::json!({ "id": id }));
    read_note(&space, &date, &id).await
}

/// The space a note or a page of the open space moves to: any other.
pub(crate) fn move_target(state: &State<'_, AppState>, name: &str) -> Result<Arc<Space>> {
    let to = state.space_or_open(Some(name))?;
    if Arc::ptr_eq(&to, &state.space()?) {
        return Err(format!("it is already in {name}").into());
    }
    Ok(to)
}

/// The id a note or a page moving to `space` onto `date` takes there: its
/// own, unless that day already holds it, as a move cut short by a crash
/// leaves it in both spaces.
pub(crate) async fn free_id(space: &Space, date: &str, id: &str) -> String {
    let contents = tokio::fs::read_to_string(day_path(&space.root, date))
        .await
        .unwrap_or_default();
    let taken = daily_file::parse_notes(&contents, date, "")
        .iter()
        .any(|note| note.id == id)
        || daily_file::parse_stubs(&contents)
            .iter()
            .any(|stub| stub.id == id);
    if taken {
        Ulid::generate().to_string()
    } else {
        id.to_string()
    }
}

/// Move a note of the open space to another, onto the same day at the same
/// time, with the files it links (SPEC 3.2). It is written there before it
/// leaves here, so a crash in between leaves it in both spaces, never in
/// neither.
#[tauri::command]
pub async fn move_note(
    app: AppHandle,
    state: State<'_, AppState>,
    date: String,
    id: String,
    space: String,
) -> Result<()> {
    check_date(&date)?;
    let from = state.space()?;
    let to = move_target(&state, &space)?;
    let note = read_note(&from, &date, &id).await?;

    let (body, carried) = crate::attachments::carry_async(&from.root, &to.root, &note.body).await?;
    let moved = Note {
        id: free_id(&to, &date, &id).await,
        hash: daily_file::body_hash(&body),
        body,
        ..note
    };
    state
        .writer
        .append_note(day_path(&to.root, &date), date.clone(), moved.clone())
        .await?;
    to.note_added(&state.writer, &moved).await?;

    state
        .writer
        .delete_note(day_path(&from.root, &date), id.clone())
        .await?;
    reindex_day(&state, &from, &date).await?;
    crate::attachments::drop_carried(&from, &carried);

    let _ = app.emit("note-updated", serde_json::json!({ "id": id }));
    Ok(())
}

fn is_page(space: &Space, id: &str) -> bool {
    space.index.read().is_ok_and(|idx| idx.page(id).is_some())
}

pub(crate) async fn read_note(space: &Space, date: &str, id: &str) -> Result<Note> {
    let contents = tokio::fs::read_to_string(day_path(&space.root, date)).await?;
    daily_file::parse_notes(&contents, date, &relative_day_path(date))
        .into_iter()
        .find(|note| note.id == id)
        .ok_or_else(|| format!("no note {id} in {date}").into())
}

/// After rewriting a day file: reparse the day and write the index back.
async fn reindex_day(state: &State<'_, AppState>, space: &Space, date: &str) -> Result<()> {
    space.day_changed(date)?;
    space.persist_index(&state.writer).await
}

/// Write the notes and pages a model labelled as they are written now, then
/// reparse every markdown file and replace the cache. Returns how many notes
/// the rebuilt index holds.
#[tauri::command]
pub async fn rebuild_index(app: AppHandle, state: State<'_, AppState>) -> Result<usize> {
    let space = state.space()?;
    drop_labels(&state, &space).await?;
    let rebuilding = space.clone();
    let count = blocking(move || rebuilding.rebuild()).await??;
    space.persist_index(&state.writer).await?;

    let _ = app.emit("index-rebuilt", ());
    Ok(count)
}

/// Write every block and page of the space that still carries the labels a
/// model wrote as it is written now, without them (SPEC 4.3). The rest of
/// each file is left as it is.
async fn drop_labels(state: &State<'_, AppState>, space: &Space) -> Result<()> {
    for (_, path) in index::daily_files(&space.root) {
        state
            .writer
            .rewrite(path, |existing| daily_file::drop_labels(existing?))
            .await?;
    }
    for path in index::page_files(&space.root) {
        let Some(file) = index::relative(&space.root, &path) else {
            continue;
        };
        state
            .writer
            .rewrite(path, move |existing| {
                page_file::drop_labels(existing?, &file)
            })
            .await?;
    }
    Ok(())
}

#[tauri::command]
pub fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// The notes and pages of the open space that look forward to `date`, oldest
/// first, for its day to show what was written ahead of it (SPEC 5.3).
#[tauri::command]
pub async fn notes_about(state: State<'_, AppState>, date: String) -> Result<Vec<Note>> {
    check_date(&date)?;
    let space = state.space()?;
    let found = space.read(|idx, db| {
        let mut about: Vec<&IndexEntry> = idx
            .entries()
            .filter(|entry| entry.on.as_deref() == Some(date.as_str()))
            .collect();
        about.sort_by(|a, b| (&a.date, &a.time).cmp(&(&b.date, &b.time)));
        crate::search::with_bodies(db, about)
    })?;
    Ok(found.unwrap_or_default())
}

/// Forget the day ahead read in a note (SPEC 5.3), for one read wrong. No
/// day is read in it again, even once its text changes.
#[tauri::command]
pub async fn clear_day_ahead(
    app: AppHandle,
    state: State<'_, AppState>,
    date: String,
    id: String,
) -> Result<()> {
    check_date(&date)?;
    let space = state.space()?;
    if is_page(&space, &id) {
        return crate::pages::clear_day_ahead(&app, &state, &space, &id).await;
    }
    let path = day_path(&space.root, &date);
    let cleared = id.clone();
    if !state
        .writer
        .rewrite(path, move |existing| {
            daily_file::clear_day_ahead(existing?, &cleared)
        })
        .await?
    {
        return Err(format!("no note {id} in {date}").into());
    }
    reindex_day(&state, &space, &date).await?;
    let _ = app.emit("note-updated", serde_json::json!({ "id": id }));
    Ok(())
}

/// Open a link from a note in the default browser or mail client. Only web
/// and mail links go through, so a note cannot launch files or programs.
#[tauri::command]
pub fn open_link(app: AppHandle, url: String) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let lower = url.to_ascii_lowercase();
    if !["http://", "https://", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
    {
        return Err(format!("not a web or mail link: {url}").into());
    }
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| format!("could not open {url}: {e}").into())
}
