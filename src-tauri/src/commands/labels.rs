//! What each note is about, SPEC 3.14.

use std::collections::HashMap;

use tauri::State;

use crate::embed::classify::NoteLabel;
use crate::state::AppState;
use crate::Result;

/// Every embedded note's label, by id. Empty without the embedding model.
#[tauri::command]
pub async fn note_labels(state: State<'_, AppState>) -> Result<HashMap<String, NoteLabel>> {
    Ok(state.space()?.labels())
}
