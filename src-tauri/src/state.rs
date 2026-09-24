use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use crate::enrich::download::{self, ActiveModel};
use crate::enrich::model::{Backend, ModelStatus};
use crate::enrich::worker::Wake;
use crate::settings::Settings;
use crate::spaces::{Registry, Space};
use crate::storage::writer::Writer;

pub struct AppState {
    /// The notes root for this run. A new one chosen in settings applies on
    /// the next launch, since the spaces, queues and watchers are all bound
    /// to it. Models and settings live here; notes live in the spaces.
    pub root: PathBuf,
    /// What settings.json says now, for the parts that apply live.
    pub settings: RwLock<Settings>,
    pub writer: Writer,
    /// Every space, the default one first. Each keeps its own notes, tags,
    /// categories and queue.
    pub spaces: RwLock<Vec<Arc<Space>>>,
    /// What spaces.json says: which space is open, and the default's name.
    pub registry: RwLock<Registry>,
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

/// What the model is while nothing is loaded.
pub fn resting_status(enabled: bool, on_disk: bool) -> ModelStatus {
    match (enabled, on_disk) {
        (false, _) => ModelStatus::Disabled,
        (true, true) => ModelStatus::Idle,
        (true, false) => ModelStatus::Absent,
    }
}

impl AppState {
    /// `None` while the worker is idle. The total grows as notes are queued
    /// mid-run, and a note put back to retry counts again.
    pub fn progress(&self) -> Option<Progress> {
        if !self.busy.load(Ordering::SeqCst) {
            return None;
        }
        let done = self.batch_done.load(Ordering::SeqCst);
        let queued: usize = self.all_spaces().iter().map(|s| s.queued()).sum();
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
        let status = resting_status(self.model_enabled(), self.active_model().is_some());
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

    /// False when the model is switched off in settings.
    pub fn model_enabled(&self) -> bool {
        self.settings
            .read()
            .map(|s| s.model_enabled)
            .unwrap_or(true)
    }

    pub fn use_gpu(&self) -> bool {
        self.settings.read().map(|s| s.use_gpu).unwrap_or(true)
    }

    /// The open space, which every note command acts on.
    pub fn space(&self) -> Result<Arc<Space>, String> {
        let active = self
            .registry
            .read()
            .map_err(|_| "spaces lock poisoned".to_string())?
            .active
            .clone();
        // At least one space is always there.
        self.find_space(&active)
            .or_else(|| self.spaces.read().ok()?.first().cloned())
            .ok_or_else(|| "no space is open".to_string())
    }

    pub fn find_space(&self, name: &str) -> Option<Arc<Space>> {
        self.spaces
            .read()
            .ok()?
            .iter()
            .find(|s| s.name == name)
            .cloned()
    }

    /// The open space first, so its notes are enriched before the others.
    pub fn all_spaces(&self) -> Vec<Arc<Space>> {
        let mut all = self
            .spaces
            .read()
            .map(|spaces| spaces.clone())
            .unwrap_or_default();
        if let Ok(active) = self.space() {
            all.sort_by_key(|s| !Arc::ptr_eq(s, &active));
        }
        all
    }

    pub fn model_status(&self) -> ModelStatus {
        self.model_status
            .read()
            .map(|status| status.clone())
            .unwrap_or(ModelStatus::Absent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switched_off_wins_over_a_model_on_disk() {
        assert_eq!(resting_status(false, true), ModelStatus::Disabled);
        assert_eq!(resting_status(false, false), ModelStatus::Disabled);
        assert_eq!(resting_status(true, true), ModelStatus::Idle);
        assert_eq!(resting_status(true, false), ModelStatus::Absent);
    }
}
