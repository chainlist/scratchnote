//! What the embedder is doing, for the status bar of a dev build: the model
//! loading, a pass embedding notes, and how the last pass went. Each change
//! is kept for a view opened later and sent out as `embedder-activity`.

use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Activity {
    /// No pass has run yet.
    #[default]
    Waiting,
    /// No embedding model is installed.
    NoModel,
    Loading,
    Failed { error: String },
    Embedding {
        space: String,
        done: usize,
        total: usize,
    },
    /// Placing the notes in threads, then on the map, after embedding.
    Threads { space: String },
    Map { space: String },
    /// The last pass is over: how many notes it embedded, how long it took
    /// and how many vectors the open space holds.
    #[serde(rename_all = "camelCase")]
    Idle {
        embedded: usize,
        ms: u64,
        vectors: usize,
    },
}

/// Keep `activity` and send it out.
pub fn report(app: &AppHandle, activity: Activity) {
    if let Ok(mut slot) = app.state::<AppState>().embedder_activity.lock() {
        *slot = activity.clone();
    }
    let _ = app.emit("embedder-activity", activity);
}

/// Progress through a pass's notes, sent no more than five times a second
/// so a first pass over thousands of notes does not flood the views. The
/// last note always goes out.
pub struct Progress<'a> {
    app: &'a AppHandle,
    space: &'a str,
    sent: Option<Instant>,
}

impl<'a> Progress<'a> {
    pub fn new(app: &'a AppHandle, space: &'a str) -> Self {
        Self {
            app,
            space,
            sent: None,
        }
    }

    pub fn tick(&mut self, done: usize, total: usize) {
        let due = self
            .sent
            .map_or(true, |sent| sent.elapsed() >= Duration::from_millis(200));
        if total == 0 || !(due || done == total) {
            return;
        }
        self.sent = Some(Instant::now());
        report(
            self.app,
            Activity::Embedding {
                space: self.space.to_string(),
                done,
                total,
            },
        );
    }
}
