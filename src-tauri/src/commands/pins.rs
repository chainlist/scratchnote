//! Pins on the left edge, SPEC 3.13: threads and plugin pages one click away.

use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;
use crate::storage::pins::Pin;

/// What the open space has pinned, in its order.
#[tauri::command]
pub async fn list_pins(state: State<'_, AppState>) -> Result<Vec<Pin>, String> {
    Ok(state.space()?.pins())
}

/// Pin these in place of the pins before, tidied, and resolve to them as
/// saved.
#[tauri::command]
pub async fn set_pins(
    app: AppHandle,
    state: State<'_, AppState>,
    pins: Vec<Pin>,
) -> Result<Vec<Pin>, String> {
    let space = state.space()?;
    let saved = space
        .change_pins(|old| {
            *old = pins;
            true
        })?
        .unwrap_or_default();
    pins_changed(&app, &space.name);
    Ok(saved)
}

/// Tell both windows the pins of `space` changed.
pub fn pins_changed(app: &AppHandle, space: &str) {
    let _ = app.emit("pins-changed", serde_json::json!({ "space": space }));
}
