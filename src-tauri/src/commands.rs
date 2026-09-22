//! Tauri commands. Milestone 1 covers capture and browsing only; enrichment,
//! search and editing commands arrive with their milestones.

use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use ulid::Ulid;

use crate::state::AppState;
use crate::storage::daily_file::{self, Note, Status};
use crate::storage::day_path;

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
        file: crate::storage::relative_day_path(&date),
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

    let _ = app.emit("note-updated", serde_json::json!({ "id": note.id }));
    Ok(Some(note))
}

#[tauri::command]
pub async fn get_day(state: State<'_, AppState>, date: String) -> Result<Vec<Note>, String> {
    check_date(&date)?;
    let mut notes = read_day(&state, &date).await?;
    notes.sort_by(|a, b| a.time.cmp(&b.time));
    Ok(notes)
}

/// Days that have a file on disk, newest first. Milestone 2 replaces this
/// directory walk with the index.
#[tauri::command]
pub async fn list_days(state: State<'_, AppState>) -> Result<Vec<DaySummary>, String> {
    let notes_dir = state.settings.root.join("notes");
    let mut days = Vec::new();

    let mut years = match tokio::fs::read_dir(&notes_dir).await {
        Ok(entries) => entries,
        Err(_) => return Ok(days),
    };
    while let Some(year) = years.next_entry().await.map_err(|e| e.to_string())? {
        if !year.path().is_dir() {
            continue;
        }
        let mut files = tokio::fs::read_dir(year.path())
            .await
            .map_err(|e| e.to_string())?;
        while let Some(file) = files.next_entry().await.map_err(|e| e.to_string())? {
            let path = file.path();
            let Some(date) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if path.extension().and_then(|e| e.to_str()) != Some("md") || check_date(date).is_err()
            {
                continue;
            }
            let date = date.to_string();
            let count = read_day(&state, &date).await?.len();
            days.push(DaySummary { date, count });
        }
    }

    days.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(days)
}

#[tauri::command]
pub fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

async fn read_day(state: &State<'_, AppState>, date: &str) -> Result<Vec<Note>, String> {
    let path = day_path(&state.settings.root, date);
    let contents = match tokio::fs::read_to_string(&path).await {
        Ok(contents) => contents,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    Ok(daily_file::parse_notes(
        &contents,
        date,
        &crate::storage::relative_day_path(date),
    ))
}

/// Guards the `YYYY-MM-DD` shape the file layout is built on, so a bad date
/// cannot reach into the filesystem.
fn check_date(date: &str) -> Result<(), String> {
    let ok = date.len() == 10
        && date.as_bytes()[4] == b'-'
        && date.as_bytes()[7] == b'-'
        && date.char_indices().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        });
    if ok {
        Ok(())
    } else {
        Err(format!("not a YYYY-MM-DD date: {date}"))
    }
}

#[cfg(test)]
mod tests {
    use super::check_date;

    #[test]
    fn accepts_a_well_formed_date() {
        assert!(check_date("2026-09-22").is_ok());
    }

    #[test]
    fn rejects_anything_that_could_escape_the_notes_directory() {
        for bad in ["", "2026-9-22", "2026/09/22", "../../etc", "2026-09-2x"] {
            assert!(check_date(bad).is_err(), "{bad} should be rejected");
        }
    }
}
