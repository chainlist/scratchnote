//! Unload the model after it has sat unused for a while, SPEC 5.1.
//!
//! A loaded model holds gigabytes of RAM for a note that may not come for
//! hours. The next job loads it again lazily, which costs a few seconds once.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

/// Often enough that a ten minute limit is honoured to within a few percent.
const TICK: Duration = Duration::from_secs(30);

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
                .map(|s| s.idle_unload_minutes)
                .unwrap_or(0);

            if should_unload(loaded, idle_for, limit) {
                log::info!(
                    "unloading the model after {} idle minutes",
                    idle_for.as_secs() / 60
                );
                let status = state.unload_model();
                let _ = app.emit("model-status", &status);
            }
        }
    });
}

/// The worker stamps `last_used` at the start and end of every job, and a job
/// is cut off at 30 seconds, so a job in progress never looks idle.
fn should_unload(loaded: bool, idle_for: Duration, limit_minutes: u32) -> bool {
    loaded && limit_minutes > 0 && idle_for >= Duration::from_secs(u64::from(limit_minutes) * 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: Duration = Duration::from_secs(60);

    #[test]
    fn unloads_once_the_limit_has_passed() {
        assert!(!should_unload(true, 9 * MINUTE, 10));
        assert!(should_unload(true, 10 * MINUTE, 10));
        assert!(should_unload(true, 60 * MINUTE, 10));
    }

    #[test]
    fn zero_keeps_the_model_loaded() {
        assert!(!should_unload(true, 600 * MINUTE, 0));
    }

    #[test]
    fn nothing_to_do_when_nothing_is_loaded() {
        assert!(!should_unload(false, 600 * MINUTE, 10));
    }
}
