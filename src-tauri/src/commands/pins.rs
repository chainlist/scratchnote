//! Pins on the left edge, SPEC 3.13: threads and plugin pages one click away.

use tauri::{AppHandle, State};

use crate::events;
use crate::state::AppState;
use crate::storage::pins::Pin;
use crate::Result;

/// What the open space has pinned, in its order.
#[tauri::command]
pub async fn list_pins(state: State<'_, AppState>) -> Result<Vec<Pin>> {
    Ok(state.space()?.pins())
}

/// Pin these in place of the pins before, tidied, and resolve to them as
/// saved.
#[tauri::command]
pub async fn set_pins(
    app: AppHandle,
    state: State<'_, AppState>,
    pins: Vec<Pin>,
) -> Result<Vec<Pin>> {
    let space = state.space()?;
    let saved = space
        .change_pins(|old| {
            *old = pins;
            true
        })?
        .unwrap_or_default();
    events::pins_changed(&app, &space.name);
    Ok(saved)
}
