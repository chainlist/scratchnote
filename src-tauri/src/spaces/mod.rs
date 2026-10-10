//! Spaces: separate sets of notes, each with its own index.
//!
//! Every space is a folder under `spaces/`, named after the space, so a folder
//! made there by hand is a space too. `.scratchnote/spaces.json` at the root
//! records only what the folders cannot: which space is open.

mod decisions;
mod migrate;
mod registry;
mod text;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock};

use crate::embed::classify::NoteLabel;
use crate::embed::map::Map;
use crate::embed::sync::Wake;
use crate::embed::threads::Threads;
use crate::embed::vectors::Vectors;
use crate::storage::index::{self, Index};
use crate::storage::markdown::collapse_spaces;
use crate::storage::paths;
use crate::storage::search_db::SearchDb;
use crate::storage::space_db::SpaceDb;
use crate::Result;
use decisions::ThreadUndo;
pub use migrate::migrate_root;
pub use registry::{registry_path, Registry};

/// Made on first launch, when there is no space yet.
pub const FIRST_NAME: &str = "Personal";
/// Long enough for any sensible name, short enough for the sidebar.
const MAX_NAME: usize = 40;

pub fn spaces_dir(root: &Path) -> PathBuf {
    root.join("spaces")
}

/// The folders under `spaces/` whose names are usable as space names, sorted
/// the way the sidebar lists them.
pub fn discover(root: &Path) -> Vec<(String, PathBuf)> {
    let mut found: Vec<(String, PathBuf)> = std::fs::read_dir(spaces_dir(root))
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .filter_map(|e| {
                    let name = e.file_name().to_str()?.to_string();
                    check_name(&name).ok().filter(|n| *n == name)?;
                    Some((name, e.path()))
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort_by_key(|(name, _)| name.to_lowercase());
    found
}

/// A space name is also a folder name, so on top of being one line and not
/// too long it must be something every platform accepts as a folder.
/// Returns the name tidied: surrounding and repeated whitespace dropped.
pub fn check_name(raw: &str) -> Result<String> {
    let name = collapse_spaces(raw);
    if name.is_empty() {
        return Err("a space needs a name".into());
    }
    if name.chars().count() > MAX_NAME {
        return Err(format!("keep the name under {MAX_NAME} characters").into());
    }
    if let Some(c) = name.chars().find(|c| paths::forbidden(*c)) {
        return Err(format!("a space name cannot contain {c}").into());
    }
    if name.starts_with('.') || name.ends_with('.') {
        return Err("a space name cannot start or end with a dot".into());
    }
    // Windows refuses these as file names, with or without an extension.
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    if reserved {
        return Err(format!("{name} is a name Windows keeps for itself").into());
    }
    Ok(name)
}

/// Every space under the root, as `discover` finds them, or the first one,
/// made on the first launch, when there is none.
pub fn discover_or_first(root: &Path) -> Vec<(String, PathBuf)> {
    let found = discover(root);
    if !found.is_empty() {
        return found;
    }
    let path = spaces_dir(root).join(FIRST_NAME);
    if let Err(e) = std::fs::create_dir_all(path.join("notes")) {
        log::error!("could not create {}: {e}", path.display());
    }
    vec![(FIRST_NAME.to_string(), path)]
}

/// One space, and its cache while it is open. Only the open space is read
/// into memory and watched (SPEC 4.6).
pub struct Space {
    pub name: String,
    pub root: PathBuf,
    /// The derived cache from SPEC 4.4, or `Index::closed()` while the space
    /// is not open. Guards are held only for the length of a read or a swap,
    /// never across an await.
    pub index: RwLock<Index>,
    /// The notes' text (SPEC 6), `None` while the space is not open. Taken
    /// after `index` when both are needed, never the other way round, so a
    /// read holding both cannot meet a write holding them the other way.
    pub search: Mutex<Option<SearchDb>>,
    /// The note embeddings search by meaning reads. `None` until the embed task
    /// first runs with a model, which loads them from `db`.
    pub vectors: Mutex<Option<Vectors>>,
    /// The thread each note is in (SPEC 6.4), placed from the vectors.
    /// `None` until the embed task first runs with a model, which loads it
    /// from `db`. Taken after `vectors` when both are needed.
    pub threads: Mutex<Option<Threads>>,
    /// Held through a placement in threads, so that two never run at once.
    /// The threads themselves are held only to copy and to swap.
    pub placing: Mutex<()>,
    /// Where each note sits on the map of the space, laid out from the
    /// vectors. `None` until the embed task first runs with a model, which
    /// loads it from `db`. Taken after `vectors`, and never with `threads`.
    pub map: Mutex<Option<Map>>,
    /// Each note's label (SPEC 3.14) with the body hash it was read from,
    /// kept between calls so only notes embedded since are labelled again.
    /// Taken after `vectors`.
    labels: Mutex<HashMap<String, (String, NoteLabel)>>,
    /// Where the vectors, the placements, the map and the thread edits are
    /// saved, `None` while the space is not open. Taken after `vectors`,
    /// `threads` and `map` when held with any.
    pub db: Mutex<Option<SpaceDb>>,
    /// Nudges the embed task whenever the index changes.
    embed_wake: Wake,
    /// Pages open in the editor. The model does not embed them until the
    /// page view closes, so autosaves do not run it again (SPEC 3.5).
    editing: Mutex<HashSet<String>>,
    /// Held only to keep it alive; dropping it stops watching.
    pub watcher: Mutex<Option<notify::RecommendedWatcher>>,
    /// Set once the space is deleted or renamed away, so nothing still in
    /// hand for it writes files back into the old folder.
    retired: AtomicBool,
    /// The last change to threads that can be taken back, kept only while
    /// the app runs.
    undo: Mutex<Option<ThreadUndo>>,
}

impl Space {
    /// A space as listed, not open: nothing of it is read yet.
    pub fn new(name: &str, root: PathBuf, embed_wake: Wake) -> Self {
        Self {
            name: name.to_string(),
            index: RwLock::new(Index::closed()),
            search: Mutex::new(None),
            vectors: Mutex::new(None),
            threads: Mutex::new(None),
            placing: Mutex::new(()),
            map: Mutex::new(None),
            labels: Mutex::new(HashMap::new()),
            db: Mutex::new(None),
            watcher: Mutex::new(None),
            retired: AtomicBool::new(false),
            root,
            embed_wake,
            editing: Mutex::new(HashSet::new()),
            undo: Mutex::new(None),
        }
    }

    /// A space read from disk and open. The flag says the index on disk is
    /// out of date and should be written back.
    #[cfg(test)]
    pub fn open(name: &str, root: PathBuf, embed_wake: Wake) -> (Self, bool) {
        let space = Self::new(name, root, embed_wake);
        let stale = space.load();
        (space, stale)
    }

    /// Read the space's notes into memory, as opening it does. The flag says
    /// the index on disk is out of date and should be written back.
    pub fn load(&self) -> bool {
        let mut db = SearchDb::open(&self.root).unwrap_or_else(|e| {
            log::error!(
                "could not open the search.db of {}, holding the text in memory: {e}",
                self.name
            );
            SearchDb::in_memory().expect("an in-memory database")
        });
        crate::startup::step(format!("Opening the search.db of {}", self.name));
        let (loaded, stale) = index::load(&self.root, &mut db);
        log::info!("space {} holds {} notes", self.name, loaded.len());
        crate::startup::step(format!(
            "Reading the index of {} ({} notes)",
            self.name,
            loaded.len()
        ));

        if let Ok(mut index) = self.index.write() {
            *index = loaded;
        }
        if let Ok(mut search) = self.search.lock() {
            *search = Some(db);
        }
        let db = SpaceDb::open(&self.root).unwrap_or_else(|e| {
            log::error!(
                "could not open the space.db of {}, saving nothing it holds: {e}",
                self.name
            );
            SpaceDb::in_memory().expect("an in-memory database")
        });
        crate::startup::step(format!("Opening the space.db of {}", self.name));
        if let Ok(mut slot) = self.db.lock() {
            *slot = Some(db);
        }
        // Its notes may have been written with no model, or by another one.
        self.embed_wake.notify_one();
        stale
    }

    /// Let go of the notes, the vectors and the watcher, as leaving the space
    /// does. Returns how many notes the space held, for the switcher to show
    /// meanwhile, and `None` if it was not open.
    pub fn unload(&self) -> Option<usize> {
        let count = self.index.write().ok().and_then(|mut index| {
            let held = std::mem::replace(&mut *index, Index::closed());
            (!held.is_closed()).then(|| held.len())
        });
        if let Ok(mut search) = self.search.lock() {
            *search = None;
        }
        if let Ok(mut vectors) = self.vectors.lock() {
            *vectors = None;
        }
        if let Ok(mut threads) = self.threads.lock() {
            *threads = None;
        }
        if let Ok(mut map) = self.map.lock() {
            *map = None;
        }
        if let Ok(mut db) = self.db.lock() {
            *db = None;
        }
        if let Ok(mut watcher) = self.watcher.lock() {
            *watcher = None;
        }
        count
    }

    pub fn is_open(&self) -> bool {
        self.index.read().is_ok_and(|index| !index.is_closed())
    }

    /// Stop watching and writing. The folder itself is the caller's to move
    /// or delete.
    pub fn retire(&self) {
        self.retired.store(true, Ordering::SeqCst);
        if let Ok(mut watcher) = self.watcher.lock() {
            *watcher = None;
        }
        // search.db and space.db are held open too, which would keep
        // Windows from moving the folder.
        if let Ok(mut search) = self.search.lock() {
            *search = None;
        }
        if let Ok(mut db) = self.db.lock() {
            *db = None;
        }
    }

    pub fn is_retired(&self) -> bool {
        self.retired.load(Ordering::SeqCst)
    }

    /// A page is being edited: the model leaves it alone until `release`.
    pub fn hold(&self, id: &str) {
        if let Ok(mut editing) = self.editing.lock() {
            editing.insert(id.to_string());
        }
    }

    /// The page view closed. True when the page was held.
    pub fn release(&self, id: &str) -> bool {
        self.editing
            .lock()
            .is_ok_and(|mut editing| editing.remove(id))
    }

    /// The pages being edited, for the embed task to skip.
    pub fn held(&self) -> HashSet<String> {
        self.editing
            .lock()
            .map(|editing| editing.clone())
            .unwrap_or_default()
    }

    /// `None` while the space is not open, which holds no notes to count.
    pub fn note_count(&self) -> Option<usize> {
        self.index
            .read()
            .ok()
            .filter(|idx| !idx.is_closed())
            .map(|idx| idx.len())
    }

    /// Notes that stand out for a query, best first, per `Vectors::related`.
    /// Empty until the embed task has loaded the vectors.
    pub fn related(&self, query: &[f32], k: usize, min_lead: f32) -> Vec<(String, f32)> {
        self.vectors
            .lock()
            .ok()
            .and_then(|vectors| Some(vectors.as_ref()?.related(query, k, min_lead)))
            .unwrap_or_default()
    }

    /// Notes close to this one, best first, per `Vectors::similar`. Empty
    /// until the embed task has loaded the vectors.
    pub fn similar(&self, id: &str, k: usize, min_score: f32) -> Vec<(String, f32)> {
        self.vectors
            .lock()
            .ok()
            .and_then(|vectors| Some(vectors.as_ref()?.similar(id, k, min_score)))
            .unwrap_or_default()
    }

    /// Every embedded note's label (SPEC 3.14). Empty until the embed task
    /// has loaded the vectors, or when they come from another embedding
    /// recipe than the classifier's.
    pub fn labels(&self) -> HashMap<String, NoteLabel> {
        let Ok(vectors) = self.vectors.lock() else {
            return HashMap::new();
        };
        let Ok(mut kept) = self.labels.lock() else {
            return HashMap::new();
        };
        match vectors.as_ref() {
            Some(vectors) => crate::embed::classify::label_notes(vectors, &mut kept),
            None => {
                kept.clear();
                HashMap::new()
            }
        }
    }

    /// Notes close to a vector that need not be stored, such as a draft's,
    /// best first, per `Vectors::closest`. Empty until the embed task has
    /// loaded the vectors.
    pub fn closest(
        &self,
        query: &[f32],
        exclude: Option<&str>,
        k: usize,
        min_score: f32,
    ) -> Vec<(String, f32)> {
        self.vectors
            .lock()
            .ok()
            .and_then(|vectors| Some(vectors.as_ref()?.closest(query, exclude, k, min_score)))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;
