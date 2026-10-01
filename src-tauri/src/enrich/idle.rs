//! Unload the chat model after it has sat unused for a while, SPEC 5.1.
//!
//! A loaded model holds gigabytes of RAM for a note that may not come for
//! hours. The next job loads it again lazily, which costs a few seconds once.
//! The embedding model stays: it is small, and search by meaning, recall and
//! every new note need it.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

/// Often enough that a limit of 30 seconds is honoured to within a few.
const TICK: Duration = Duration::from_secs(5);

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(TICK).await;
            let state = app.state::<AppState>();

            let loaded = state.backend.read().is_ok_and(|slot| slot.is_some());
            let idle_for = state
                .last_used
                .lock()
                .map(|last| last.elapsed())
                .unwrap_or_default();
            let limit = state
                .settings
                .read()
                .map(|s| s.idle_unload_seconds)
                .unwrap_or(0);

            if should_unload(loaded, idle_for, limit) {
                log::info!(
                    "unloading the model after {} idle seconds",
                    idle_for.as_secs()
                );
                let status = state.unload_chat_model();
                let _ = app.emit("model-status", &status);
            }
        }
    });
}

/// The worker stamps `last_used` at the start and end of every job, and a job
/// is cut off at 30 seconds, so a job in progress never looks idle.
fn should_unload(loaded: bool, idle_for: Duration, limit_seconds: u32) -> bool {
    loaded && limit_seconds > 0 && idle_for >= Duration::from_secs(u64::from(limit_seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    #[test]
    fn unloads_once_the_limit_has_passed() {
        assert!(!should_unload(true, 29 * SECOND, 30));
        assert!(should_unload(true, 30 * SECOND, 30));
        assert!(should_unload(true, 3600 * SECOND, 600));
    }

    #[test]
    fn zero_keeps_the_model_loaded() {
        assert!(!should_unload(true, 36_000 * SECOND, 0));
    }

    #[test]
    fn nothing_to_do_when_nothing_is_loaded() {
        assert!(!should_unload(false, 36_000 * SECOND, 600));
    }
}
