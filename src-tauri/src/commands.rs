//! Tauri commands. Milestones 1 and 2 cover capture, browsing and the index;
//! enrichment, tags and search arrive with their milestones.

use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use ulid::Ulid;

use crate::state::AppState;
use crate::storage::daily_file::{self, Note, Status};
use crate::storage::index::{self, IndexEntry};
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
        .append_note(day_path(&state.settings.root, &date), date, note.clone())
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
        .append_index_line(index::index_path(&state.settings.root), line)
        .await?;

    let _ = app.emit("note-updated", serde_json::json!({ "id": note.id }));
    Ok(Some(note))
}

/// Read straight from the markdown, because the index deliberately carries no
/// bodies and the day view shows them.
#[tauri::command]
pub async fn get_day(state: State<'_, AppState>, date: String) -> Result<Vec<Note>, String> {
    check_date(&date)?;
    let path = day_path(&state.settings.root, &date);
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
    let path = day_path(&state.settings.root, &date);
    if !state.writer.delete_note(path.clone(), id.clone()).await? {
        return Err(format!("no note {id} in {date}"));
    }

    // A delete rewrites the day file, so the day's entries are reparsed and
    // the whole index written back (SPEC 4.4).
    let entries = index::parse_day(&path, &date);
    let contents = {
        let mut idx = state
            .index
            .write()
            .map_err(|_| "index lock poisoned".to_string())?;
        idx.replace_day(&date, entries);
        idx.to_jsonl()
    };
    state
        .writer
        .write_index(index::index_path(&state.settings.root), contents)
        .await?;

    let _ = app.emit("note-updated", serde_json::json!({ "id": id }));
    Ok(())
}

/// Reparse every markdown file and replace the cache. Returns how many notes
/// the rebuilt index holds.
#[tauri::command]
pub async fn rebuild_index(app: AppHandle, state: State<'_, AppState>) -> Result<usize, String> {
    let root = state.settings.root.clone();
    let rebuilt = tauri::async_runtime::spawn_blocking(move || index::rebuild(&root))
        .await
        .map_err(|e| e.to_string())?;

    let count = rebuilt.len();
    let contents = rebuilt.to_jsonl();
    *state
        .index
        .write()
        .map_err(|_| "index lock poisoned".to_string())? = rebuilt;

    state
        .writer
        .write_index(index::index_path(&state.settings.root), contents)
        .await?;

    let _ = app.emit("index-rebuilt", ());
    Ok(count)
}

#[tauri::command]
pub fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}
