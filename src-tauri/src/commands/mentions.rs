//! Mentions, SPEC 3.10: the names the notes mention, and the notes naming one.

use tauri::State;

use crate::mentions::{self, MentionNotes, MentionSummary};
use crate::state::AppState;

/// Every name the open space mentions, most mentioned first. Asked for
/// another space, as the capture window does for the one it picked, none:
/// only the open one is read.
#[tauri::command]
pub fn list_mentions(
    state: State<'_, AppState>,
    space: Option<String>,
) -> Result<Vec<MentionSummary>, String> {
    let open = state.space()?;
    if space.is_some_and(|name| name != open.name) {
        return Ok(Vec::new());
    }
    Ok(open.read(mentions::list)?.unwrap_or_default())
}

/// The notes and pages that mention `name`, newest first. Case does not count.
#[tauri::command]
pub fn notes_mentioning(state: State<'_, AppState>, name: String) -> Result<MentionNotes, String> {
    let found = state
        .space()?
        .read(|idx, db| mentions::notes(idx, db, &name))?;
    Ok(found.unwrap_or_default())
}
