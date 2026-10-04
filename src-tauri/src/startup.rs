//! What the app does at launch before its window loads, step by step, for
//! the startup details a dev build shows (Settings > About). Each step is
//! timed from the end of the one before. Steps after the launch, such as
//! opening another space later, are not kept.

use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub step: String,
    pub ms: f64,
}

/// The launch as the window reads it.
#[derive(Debug, Clone, Serialize)]
pub struct Launch {
    /// When the process began, in ms since the Unix epoch, which the window
    /// sets its own times against.
    pub began: f64,
    pub steps: Vec<Step>,
}

struct Recording {
    launch: Launch,
    last: Instant,
    /// Set once the setup is over: later steps are not the launch's.
    done: bool,
}

static RECORDING: Mutex<Option<Recording>> = Mutex::new(None);

/// Start the clock, first thing in the process.
pub fn begin() {
    let began = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |since| since.as_secs_f64() * 1000.0);
    if let Ok(mut recording) = RECORDING.lock() {
        *recording = Some(Recording {
            launch: Launch {
                began,
                steps: Vec::new(),
            },
            last: Instant::now(),
            done: false,
        });
    }
}

/// The step that just ended, timed since the one before.
pub fn step(name: impl Into<String>) {
    let Ok(mut recording) = RECORDING.lock() else {
        return;
    };
    let Some(recording) = recording.as_mut().filter(|r| !r.done) else {
        return;
    };
    let now = Instant::now();
    recording.launch.steps.push(Step {
        step: name.into(),
        ms: (now - recording.last).as_secs_f64() * 1000.0,
    });
    recording.last = now;
}

/// The launch is over: steps from here on are not kept.
pub fn done() {
    if let Ok(mut recording) = RECORDING.lock() {
        if let Some(recording) = recording.as_mut() {
            recording.done = true;
        }
    }
}

/// The launch's steps, once: a window reloaded later did not wait on them.
#[tauri::command]
pub fn launch_steps() -> Option<Launch> {
    let mut recording = RECORDING.lock().ok()?;
    if !recording.as_ref()?.done {
        return None;
    }
    recording.take().map(|recording| recording.launch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launch_is_read_once_and_keeps_only_its_own_steps() {
        begin();
        step("one");
        step("two");
        assert!(launch_steps().is_none(), "not over yet");
        done();
        step("after");

        let launch = launch_steps().expect("the launch");
        // Tests run alongside may open spaces, which add steps of their own.
        let names: Vec<&str> = launch.steps.iter().map(|s| s.step.as_str()).collect();
        assert!(names.contains(&"one") && names.contains(&"two"));
        assert!(!names.contains(&"after"));
        assert!(launch.began > 0.0);
        assert!(launch_steps().is_none(), "a reloaded window gets none");
    }
}
