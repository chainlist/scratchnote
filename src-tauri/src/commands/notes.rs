//! Notes: capture, the day view, edits, and the index behind them. What
//! each does to the files is `crate::notes`.

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::events;
use crate::notes::{self, move_target};
use crate::state::{read_lock, AppState};
use crate::storage::check_date;
use crate::storage::daily_file::Note;
use crate::storage::index::IndexEntry;
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
    let note = notes::add(&state.writer, &space, body, date).await?;
    events::note_updated(&app, &note.id);
    Ok(Some(note))
}

/// A day's notes and pages, by time (`notes::day`).
#[tauri::command]
pub async fn get_day(state: State<'_, AppState>, date: String) -> Result<Vec<Note>> {
    check_date(&date)?;
    let space = state.space()?;
    notes::day(&space, &date).await
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
    let note = space.read(|idx, db| match idx.get(&id) {
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
    let notes = space
        .read(|idx, db| crate::search::with_bodies(db, idx.pick(ids.iter().map(String::as_str))))?;
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
    notes::delete(&state.writer, &space, &date, &id).await?;
    events::note_updated(&app, &id);
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
    let (note, changed) = notes::update(&state.writer, &space, &date, &id, body).await?;
    if changed {
        events::note_updated(&app, &id);
    }
    Ok(note)
}

/// Move a note of the open space to another, onto the same day at the same
/// time, with the files it links (SPEC 3.2).
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
    notes::move_to(&state.writer, &from, &to, &date, &id).await?;
    events::note_updated(&app, &id);
    Ok(())
}

/// Write the notes and pages a model labelled as they are written now, then
/// reparse every markdown file and replace the cache. Returns how many notes
/// the rebuilt index holds.
#[tauri::command]
pub async fn rebuild_index(app: AppHandle, state: State<'_, AppState>) -> Result<usize> {
    let count = notes::rebuild(&state.writer, &state.space()?).await?;
    events::index_rebuilt(&app);
    Ok(count)
}

#[tauri::command]
pub fn today() -> String {
    crate::storage::today()
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

/// Forget the day ahead read in a note or a page (SPEC 5.3), for one read
/// wrong. No day is read in it again, even once its text changes.
#[tauri::command]
pub async fn clear_day_ahead(
    app: AppHandle,
    state: State<'_, AppState>,
    date: String,
    id: String,
) -> Result<()> {
    check_date(&date)?;
    let space = state.space()?;
    notes::clear_day_ahead(&state.writer, &space, &date, &id).await?;
    events::note_updated(&app, &id);
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
