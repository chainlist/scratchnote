use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use crate::enrich::download::{self, ActiveModel};
use crate::enrich::model::{Backend, ModelStatus};
use crate::enrich::normalize::Vocabulary;
use crate::enrich::queue::Queue;
use crate::enrich::worker::Wake;
use crate::settings::Settings;
use crate::storage::index::{self, Index};
use crate::storage::{categories, tags};
use crate::storage::writer::Writer;

pub struct AppState {
    /// The notes root for this run. A new one chosen in settings applies on
    /// the next launch, since the index, queue and watcher are all bound to it.
    pub root: PathBuf,
    /// What settings.json says now, for the parts that apply live.
    pub settings: RwLock<Settings>,
    pub writer: Writer,
    /// The derived cache from SPEC 4.4. Guards are held only for the length of
    /// a read or a swap, never across an await.
    pub index: RwLock<Index>,
    /// The user's half of `tags.json` (SPEC 4.5), read at startup and again on
    /// a rebuild.
    pub aliases: RwLock<HashMap<String, String>>,
    /// Pending enrichment jobs (SPEC 5.6).
    pub queue: Mutex<Queue>,
    /// `None` until a model is loaded. Absent is a normal state: capture,
    /// browsing and search all work without one (SPEC 11).
    pub backend: RwLock<Option<Arc<dyn Backend>>>,
    pub model_status: RwLock<ModelStatus>,
    /// Set while a new model file is being moved into place. The old one has
    /// to be unloaded for that on Windows, and this keeps the worker from
    /// loading it straight back.
    pub swapping: AtomicBool,
    /// When the worker last started or finished a job, for the idle unload in
    /// SPEC 5.1.
    pub last_used: Mutex<Instant>,
    /// True while the worker has a job in hand, loading the model included.
    pub busy: AtomicBool,
    /// Notes finished since the worker last went idle, for the `3/142` the
    /// status bar shows.
    pub batch_done: AtomicUsize,
    /// Nudges the worker when a job is queued or a model becomes available.
    pub wake: Wake,
}

/// Where the worker is in the current stretch of work: `current` of `total`,
/// both counting the note in hand.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Progress {
    pub current: usize,
    pub total: usize,
}

impl AppState {
    /// `None` while the worker is idle. The total grows as notes are queued
    /// mid-run, and a note put back to retry counts again.
    pub fn progress(&self) -> Option<Progress> {
        if !self.busy.load(Ordering::SeqCst) {
            return None;
        }
        let done = self.batch_done.load(Ordering::SeqCst);
        let queued = self.queue.lock().map(|queue| queue.len()).unwrap_or(0);
        Some(Progress {
            current: done + 1,
            total: done + 1 + queued,
        })
    }

    /// Drop the loaded model and say what state that leaves. A job still
    /// holding it finishes first, since it owns its own reference.
    pub fn unload_model(&self) -> ModelStatus {
        if let Ok(mut slot) = self.backend.write() {
            *slot = None;
        }
        let status = if self.active_model().is_some() {
            ModelStatus::Idle
        } else {
            ModelStatus::Absent
        };
        self.set_model_status(status.clone());
        status
    }

    pub fn mark_used(&self) {
        if let Ok(mut last) = self.last_used.lock() {
            *last = Instant::now();
        }
    }

    pub fn set_model_status(&self, status: ModelStatus) {
        if let Ok(mut current) = self.model_status.write() {
            *current = status;
        }
    }

    /// The model file enrichment should load, per the current settings.
    pub fn active_model(&self) -> Option<ActiveModel> {
        let settings = self.settings.read().ok()?;
        download::active_model(
            &self.root,
            settings.model_variant,
            settings.model_path.as_deref(),
        )
    }

    pub fn use_gpu(&self) -> bool {
        self.settings.read().map(|s| s.use_gpu).unwrap_or(true)
    }

    pub fn aliases(&self) -> HashMap<String, String> {
        self.aliases
            .read()
            .map(|aliases| aliases.clone())
            .unwrap_or_default()
    }

    /// What enrichment normalises against (SPEC 5.5).
    pub fn vocabulary(&self) -> Vocabulary {
        Vocabulary {
            counts: self
                .index
                .read()
                .map(|idx| idx.tag_counts())
                .unwrap_or_default(),
            aliases: self.aliases(),
            categories: categories::load(&self.root),
        }
    }

    /// Write `index.jsonl` and `tags.json` from what is in memory. The tag
    /// counts are derived from the index, so they are rewritten whenever it is
    /// and the two never drift apart.
    pub async fn persist_index(&self) -> Result<(), String> {
        let (jsonl, counts) = {
            let idx = self
                .index
                .read()
                .map_err(|_| "index lock poisoned".to_string())?;
            (idx.to_jsonl(), idx.tag_counts())
        };
        let root = &self.root;
        self.writer
            .write_index(index::index_path(root), jsonl)
            .await?;
        self.writer
            .write_index(
                tags::tags_path(root),
                tags::render(&counts, &self.aliases()),
            )
            .await
    }

    pub fn model_status(&self) -> ModelStatus {
        self.model_status
            .read()
            .map(|status| status.clone())
            .unwrap_or(ModelStatus::Absent)
    }
}
