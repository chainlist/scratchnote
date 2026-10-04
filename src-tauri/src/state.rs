use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use crate::embed::sync::Wake;
use crate::embed::Embedder;
use crate::settings::Settings;
use crate::spaces::{Registry, Space};
use crate::storage::writer::Writer;

pub struct AppState {
    /// The notes root for this run. A new one chosen in settings applies on
    /// the next launch, since the spaces and their watchers are all bound
    /// to it. The model and settings live here; notes live in the spaces.
    pub root: PathBuf,
    /// What settings.json says now, for the parts that apply live.
    pub settings: RwLock<Settings>,
    pub writer: Writer,
    /// Every space, the default one first. Each keeps its own notes.
    pub spaces: RwLock<Vec<Arc<Space>>>,
    /// What spaces.json says: which space is open, and the default's name.
    pub registry: RwLock<Registry>,
    /// `None` until an embedding model is loaded, and then kept until the
    /// app quits. Read through `embed::embedder`.
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

    /// The open space first, so its notes are embedded before the others.
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
