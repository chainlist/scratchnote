use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::Instant;

use crate::embed::Embedder;
use crate::enrich::download::{self, ActiveModel};
use crate::enrich::language::{self, Language};
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
    /// Every space, the default one first. Each keeps its own notes,
    /// categories and queue.
    pub spaces: RwLock<Vec<Arc<Space>>>,
    /// What spaces.json says: which space is open, and the default's name.
    pub registry: RwLock<Registry>,
    /// `None` until a model is loaded. Absent is a normal state: capture,
    /// browsing and search all work without one (SPEC 11).
    pub backend: RwLock<Option<Arc<dyn Backend>>>,
    /// Held while that model loads or its file is swapped, so the two never
    /// overlap and two loads never run at once (`load_once`).
    pub backend_loading: Mutex<()>,
    pub model_status: RwLock<ModelStatus>,
    /// Set while a new model file is being moved into place. The old one has
    /// to be unloaded for that on Windows, and this keeps the worker from
    /// loading it straight back.
    pub swapping: AtomicBool,
    /// When the chat model was last used, by a job or a chat, for the idle
    /// unload in SPEC 5.1. The embedding model does not count: it is never
    /// unloaded for being idle.
    pub last_used: Mutex<Instant>,
    /// True while the worker has a job in hand, loading the model included.
    pub busy: AtomicBool,
    /// Notes finished since the worker last went idle, for the `3/142` the
    /// status bar shows.
    pub batch_done: AtomicUsize,
    /// Nudges the worker when a job is queued or a model becomes available.
    pub wake: Wake,
    /// `None` until an embedding model is loaded, and then until the GPU
    /// setting changes. Read through `embed::embedder`, which neither the
    /// model switch nor the idle unload reaches.
    pub embedder: RwLock<Option<Arc<dyn Embedder>>>,
    /// Held while the embedding model loads (`load_once`).
    pub embedder_loading: Mutex<()>,
    /// Nudges the embed task when an index changes or a model becomes
    /// available.
    pub embed_wake: Wake,
    /// How far the embedding model's download is, while one runs. Kept here
    /// so a settings screen opened meanwhile shows it, and a second click
    /// does not start another.
    pub embedding_download: Mutex<Option<u8>>,
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

/// What `slot` holds, or else what `load` makes, which is stored there. One
/// load runs at a time under `loading`: a caller that comes while one runs
/// waits for it, then finds its model in the slot instead of loading another
/// copy. A load that fails leaves the slot empty for the next caller.
pub fn load_once<T: ?Sized>(
    slot: &RwLock<Option<Arc<T>>>,
    loading: &Mutex<()>,
    load: impl FnOnce() -> Option<Arc<T>>,
) -> Option<Arc<T>> {
    let held = || slot.read().ok().and_then(|model| model.clone());
    if let Some(model) = held() {
        return Some(model);
    }
    // Only `()` is guarded, so a load that panicked leaves nothing to repair.
    let _one_at_a_time = loading.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(model) = held() {
        return Some(model);
    }
    let model = load()?;
    if let Ok(mut slot) = slot.write() {
        *slot = Some(model.clone());
    }
    Some(model)
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

    /// Drop the loaded model, and the embedding model with it, and say what
    /// state that leaves: for a GPU change, the one setting both follow. A
    /// job still holding one finishes first, since it owns its own
    /// reference. Both load again lazily when next needed.
    pub fn unload_model(&self) -> ModelStatus {
        if let Ok(mut slot) = self.embedder.write() {
            *slot = None;
        }
        self.unload_chat_model()
    }

    /// `unload_model` for the chat model alone, when the switch, the model
    /// choice or its file changes, or it sits idle: none of them is the
    /// embedding model's.
    pub fn unload_chat_model(&self) -> ModelStatus {
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

    /// The language notes are labelled in, read afresh for every note so a
    /// change applies from the next one on.
    pub fn note_language(&self) -> Language {
        self.settings
            .read()
            .map(|s| language::resolve(&s.language))
            .unwrap_or(language::ENGLISH)
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

    /// The space named, or the open one when no name is given, as for a note
    /// the capture window sends to another space.
    pub fn space_or_open(&self, name: Option<&str>) -> Result<Arc<Space>, String> {
        match name {
            Some(name) => self
                .find_space(name)
                .ok_or_else(|| format!("no space {name}")),
            None => self.space(),
        }
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

    /// A load slow enough for a second caller to arrive while it runs.
    fn slow_load(loads: &AtomicUsize) -> Option<Arc<String>> {
        loads.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(300));
        Some(Arc::new("model".to_string()))
    }

    #[test]
    fn a_caller_during_a_load_waits_for_it_instead_of_loading_again() {
        let slot = RwLock::new(None);
        let loading = Mutex::new(());
        let loads = AtomicUsize::new(0);
        let (first, second) = std::thread::scope(|scope| {
            let first = scope.spawn(|| load_once(&slot, &loading, || slow_load(&loads)));
            std::thread::sleep(std::time::Duration::from_millis(50));
            let second = scope.spawn(|| load_once(&slot, &loading, || slow_load(&loads)));
            (first.join().unwrap(), second.join().unwrap())
        });
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        assert!(Arc::ptr_eq(&first.unwrap(), &second.unwrap()));
    }

    #[test]
    fn a_failed_load_leaves_the_slot_for_the_next_caller() {
        let slot = RwLock::new(None);
        let loading = Mutex::new(());
        assert!(load_once(&slot, &loading, || None::<Arc<String>>).is_none());
        let model = load_once(&slot, &loading, || Some(Arc::new("model".to_string())));
        assert_eq!(model.as_deref().map(String::as_str), Some("model"));
        // In the slot now, so nothing loads.
        let again = load_once(&slot, &loading, || panic!("loaded twice"));
        assert!(Arc::ptr_eq(&model.unwrap(), &again.unwrap()));
    }
}
