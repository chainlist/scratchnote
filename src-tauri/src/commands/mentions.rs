//! Mentions, SPEC 3.10: the names the notes mention, and the notes naming one.

use tauri::State;

use crate::mentions::{self, MentionNotes, MentionSummary};
use crate::state::AppState;
use crate::storage::pins::PinKind;

/// Every name the open space mentions, most mentioned first, those pinned
/// to the left edge marked. Asked for another space, as the capture window
/// does for the one it picked, none: only the open one is read.
#[tauri::command]
pub async fn list_mentions(
    state: State<'_, AppState>,
    space: Option<String>,
) -> Result<Vec<MentionSummary>, String> {
    let open = state.space()?;
    if space.is_some_and(|name| name != open.name) {
        return Ok(Vec::new());
    }
    let mut listed = open.read(mentions::list)?.unwrap_or_default();
    let pinned: Vec<String> = open
        .pins()
        .into_iter()
        .filter(|pin| pin.kind == PinKind::Mention)
        .map(|pin| pin.target)
        .collect();
    for mention in &mut listed {
        mention.pinned = pinned.contains(&mention.key);
    }
    Ok(listed)
}

/// The notes and pages that mention `name`, newest first. Case does not count.
#[tauri::command]
pub async fn notes_mentioning(state: State<'_, AppState>, name: String) -> Result<MentionNotes, String> {
    let found = state
        .space()?
        .read(|idx, db| mentions::notes(idx, db, &name))?;
    Ok(found.unwrap_or_default())
}
