//! The activity log, SPEC 4.11, as Settings > Activity reads it.

use chrono::Local;
use serde::Serialize;
use tauri::State;

use crate::state::AppState;
use crate::storage::activity::{self, Logged, Whose};

#[derive(Debug, Serialize)]
pub struct ActivityPage {
    pub events: Vec<Logged>,
    /// Whether older ones are left.
    pub more: bool,
}

/// A page of the open space's activity log, newest first: `limit` events
/// past the first `offset`, only `whose` when given.
#[tauri::command]
pub async fn list_activity(
    state: State<'_, AppState>,
    offset: usize,
    limit: usize,
    whose: Option<Whose>,
) -> Result<ActivityPage, String> {
    let root = state.space()?.root.clone();
    let today = Local::now().date_naive();
    let (events, more) = tauri::async_runtime::spawn_blocking(move || {
        activity::read(&root, today, whose, offset, limit)
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(ActivityPage { events, more })
}
