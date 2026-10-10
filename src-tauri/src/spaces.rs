//! Spaces: separate sets of notes, each with its own index.
//!
//! Every space is a folder under `spaces/`, named after the space, so a folder
//! made there by hand is a space too. `.scratchnote/spaces.json` at the root
//! records only what the folders cannot: which space is open.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock, RwLockWriteGuard};

use serde::{Deserialize, Serialize};

use crate::embed::classify::NoteLabel;
use crate::embed::sync::Wake;
use crate::embed::map::Map;
use crate::embed::threads::{Edits, Threads};
use crate::embed::vectors::Vectors;
use crate::state::{lock, read_lock, write_lock};
use crate::storage::day_path;
use crate::storage::markdown::collapse_spaces;
use crate::storage::paths::{self, first_free, meta_dir};
use crate::storage::daily_file::Note;
use crate::storage::index::{self, Index, IndexEntry};
use crate::storage::pins::{self, Pin};
use crate::storage::search_db::{SearchDb, Stamp};
use crate::storage::space_db::SpaceDb;
use crate::storage::writer::Writer;
use crate::{Error, Result};

/// Made on first launch, when there is no space yet.
pub const FIRST_NAME: &str = "Personal";
/// Long enough for any sensible name, short enough for the sidebar.
const MAX_NAME: usize = 40;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Registry {
    /// Name of the open space.
    pub active: String,
    /// How many notes each space held when it was last left, for the
    /// switcher to show while the space is not open, and so not counted.
    pub notes: BTreeMap<String, usize>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            active: FIRST_NAME.to_string(),
            notes: BTreeMap::new(),
        }
    }
}

impl Registry {
    /// A missing or broken file opens the first space.
    pub fn load(root: &Path) -> Self {
        match std::fs::read_to_string(registry_path(root)) {
            Ok(raw) => serde_json::from_str::<Registry>(&raw).unwrap_or_else(|e| {
                log::warn!("spaces.json is not readable ({e}), opening the first space");
                Registry::default()
            }),
            Err(_) => Registry::default(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

pub fn registry_path(root: &Path) -> PathBuf {
    meta_dir(root).join("spaces.json")
}

pub fn spaces_dir(root: &Path) -> PathBuf {
    root.join("spaces")
}

/// Before every space had a folder, the first one lived at the notes root,
/// named in `spaces.json`. Move its notes and its own files into
/// `spaces/<its name>/`, once, and point `spaces.json` at the new folder.
/// Settings, models and the trash stay at the root, shared by every space.
pub fn migrate_root(root: &Path) {
    let notes = root.join("notes");
    if !notes.is_dir() {
        return;
    }
    #[derive(Default, Deserialize)]
    #[serde(default, rename_all = "camelCase")]
    struct Old {
        active: String,
        default_name: String,
    }
    let old: Old = std::fs::read_to_string(registry_path(root))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    let wanted = check_name(&old.default_name).unwrap_or_else(|_| FIRST_NAME.to_string());
    // Folders compare without case on Windows, so "personal" would clash too.
    let taken: Vec<String> = discover(root)
        .into_iter()
        .map(|(name, _)| name.to_lowercase())
        .collect();
    let name = first_free(
        |n| paths::numbered(&wanted, None, n),
        |name| taken.contains(&name.to_lowercase()),
    );
    let to = spaces_dir(root).join(&name);

    let moved = std::fs::create_dir_all(meta_dir(&to))
        .and_then(|_| std::fs::rename(&notes, to.join("notes")));
    if let Err(e) = moved {
        log::error!(
            "could not move {} into {}: {e}",
            notes.display(),
            to.display()
        );
        return;
    }
    let from = index::index_path(root);
    if from.exists() {
        if let Err(e) = std::fs::rename(&from, index::index_path(&to)) {
            log::warn!(
                "could not move {} into {}: {e}",
                from.display(),
                to.display()
            );
        }
    }

    let active = if old.active.is_empty() || old.active == old.default_name {
        name.clone()
    } else {
        old.active
    };
    let registry = Registry {
        active,
        ..Registry::default()
    };
    if let Err(e) = std::fs::write(registry_path(root), registry.to_json()) {
        log::warn!("could not update spaces.json ({e})");
    }
    log::info!("moved the notes at the root into spaces/{name}");
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
    pub retired: AtomicBool,
    /// The last change to threads that can be taken back, kept only while
    /// the app runs.
    undo: Mutex<Option<ThreadUndo>>,
}

/// The thread edits and the pins together: what one change to threads, a
/// merge say, may change both of.
pub type Decided = (Edits, Vec<Pin>);

/// What the threads were before a change, and what the change left.
struct ThreadUndo {
    before: Decided,
    after: Decided,
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

    /// What the user decided about threads, none while the space is not
    /// open.
    pub fn edits(&self) -> Edits {
        match self.db.lock().as_deref() {
            Ok(Some(db)) => Edits::load(db),
            _ => Edits::default(),
        }
    }

    /// Change what the user decided about threads, read and saved under
    /// one lock so that two changes never undo each other. `change` says
    /// whether it changed anything, and nothing is saved when it did not.
    pub fn change_edits(&self, change: impl FnOnce(&mut Edits) -> bool) -> Result<bool> {
        let mut db = lock(&self.db, "space.db")?;
        let db = db.as_mut().ok_or(Error::SpaceClosed)?;
        let mut edits = Edits::load(db);
        if !change(&mut edits) {
            return Ok(false);
        }
        edits.save(db)?;
        Ok(true)
    }

    /// What is pinned to the left edge, in its order (SPEC 3.13).
    pub fn pins(&self) -> Vec<Pin> {
        match self.db.lock().as_deref() {
            Ok(Some(db)) => pins::load(db),
            _ => Vec::new(),
        }
    }

    /// Change the pins under one lock, as `change_edits` changes the edits.
    /// Returns them as saved, or `None` when `change` changed nothing.
    pub fn change_pins(
        &self,
        change: impl FnOnce(&mut Vec<Pin>) -> bool,
    ) -> Result<Option<Vec<Pin>>> {
        let mut db = lock(&self.db, "space.db")?;
        let db = db.as_mut().ok_or(Error::SpaceClosed)?;
        let mut pins = pins::load(db);
        if !change(&mut pins) {
            return Ok(None);
        }
        let pins = pins::tidy(pins);
        pins::save(db, &pins)?;
        Ok(Some(pins))
    }

    /// Change what the user decided about threads, as `change_edits` does,
    /// and remember the change for `undo_threads` when there was one. Says
    /// whether there was.
    pub fn decide(&self, change: impl FnOnce(&mut Edits) -> bool) -> Result<bool> {
        let before = self.decided();
        let changed = self.change_edits(change)?;
        if changed {
            self.remember_change(before);
        }
        Ok(changed)
    }

    /// The thread edits and the pins as they are now.
    pub fn decided(&self) -> Decided {
        (self.edits(), self.pins())
    }

    /// Remember the change to threads made since `before`, so that
    /// `undo_threads` can take it back. It replaces the one before it.
    pub fn remember_change(&self, before: Decided) {
        let after = self.decided();
        if let Ok(mut undo) = self.undo.lock() {
            *undo = (before != after).then_some(ThreadUndo { before, after });
        }
    }

    /// Put the thread edits and the pins back as they were before the last
    /// change remembered, once, and only while nothing has changed them
    /// since: a later change is never lost to an older undo. `None` when
    /// there was nothing to take back, else whether the pins changed too.
    pub fn undo_threads(&self) -> Result<Option<bool>> {
        let Some(undo) = lock(&self.undo, "undo")?.take() else {
            return Ok(None);
        };
        if self.decided() != undo.after {
            return Ok(None);
        }
        let (edits, pins) = undo.before;
        self.change_edits(|now| {
            *now = edits;
            true
        })?;
        let pins_changed = self
            .change_pins(|now| {
                if *now == pins {
                    return false;
                }
                *now = pins;
                true
            })?
            .is_some();
        Ok(Some(pins_changed))
    }

    /// Tell the embed task the index changed. `persist_index` does it, and
    /// so does `note_added`, which appends to the index instead.
    pub fn index_changed(&self) {
        self.embed_wake.notify_one();
    }

    // The index is changed through the methods below only, so search.db
    // follows it from one place. Each writes the text first, then takes the
    // index lock for its own change; `persist_index` then writes the lot
    // back once. A space that is not open drops the change: its files are
    // then newer than both caches, so `index::load` reads them again when it
    // opens.

    /// The index to change, or `None` while the space is not open.
    fn index_to_change(&self) -> Result<Option<RwLockWriteGuard<'_, Index>>> {
        let idx = write_lock(&self.index, "index")?;
        Ok((!idx.is_closed()).then_some(idx))
    }

    /// Change the text in search.db, if the space is open. A write that fails
    /// is only logged: its file stays marked as read before it, so it is read
    /// again at the next open.
    fn write_text(&self, change: impl FnOnce(&mut SearchDb) -> Result<()>) {
        let Ok(mut search) = self.search.lock() else {
            return;
        };
        if let Some(db) = search.as_mut() {
            if let Err(e) = change(db) {
                log::warn!("could not update the search.db of {}: {e}", self.name);
            }
        }
    }

    /// A note just captured: added rather than its day reparsed, and its
    /// line appended to `index.jsonl` rather than the file rewritten, since
    /// capture has a latency budget.
    pub async fn note_added(&self, writer: &Writer, note: &Note) -> Result<()> {
        let entry = IndexEntry::from(note);
        let line = serde_json::to_string(&entry)?;
        let stamp = Stamp::of(&day_path(&self.root, &note.date));
        self.write_text(|db| db.add_note(note, stamp));
        match self.index_to_change()? {
            Some(mut idx) => idx.push(entry),
            None => return Ok(()),
        }
        writer
            .append_index_line(index::index_path(&self.root), line)
            .await?;
        self.index_changed();
        Ok(())
    }

    /// Read a day's file again after it was written, in place of what the
    /// index had for that day.
    pub fn day_changed(&self, date: &str) -> Result<()> {
        if !self.is_open() {
            return Ok(());
        }
        let path = day_path(&self.root, date);
        // Taken before the file is read, so a write in between leaves the day
        // looking older than it is, and read again, never the other way.
        let stamp = Stamp::of(&path);
        self.set_day(date, &index::parse_day(&path, date), stamp)
    }

    /// Put `notes`, parsed from a day's file as it was at `stamp`, in place
    /// of that day's.
    pub fn set_day(&self, date: &str, notes: &[Note], stamp: Option<Stamp>) -> Result<()> {
        self.write_text(|db| db.replace_day(date, notes, stamp));
        if let Some(mut idx) = self.index_to_change()? {
            idx.replace_day(date, index::entries(notes));
        }
        Ok(())
    }

    /// A page added or changed. Returns what the index had for it before.
    pub fn page_changed(&self, page: &Note) -> Result<Option<IndexEntry>> {
        let stamp = Stamp::of(&self.root.join(&page.file));
        self.write_text(|db| db.replace_page(page, stamp));
        Ok(self
            .index_to_change()?
            .and_then(|mut idx| idx.replace_page(IndexEntry::from(page))))
    }

    /// A page deleted. Returns what the index had for it.
    pub fn page_removed(&self, id: &str) -> Result<Option<IndexEntry>> {
        self.write_text(|db| db.remove_page(id));
        Ok(self
            .index_to_change()?
            .and_then(|mut idx| idx.remove_page(id)))
    }

    /// Whatever page the index had at `file`, a path relative to the root,
    /// is gone from there. Returns its entry.
    pub fn page_file_gone(&self, file: &str) -> Result<Option<IndexEntry>> {
        let gone = {
            let Some(mut idx) = self.index_to_change()? else {
                return Ok(None);
            };
            let id = idx.page_at(file).map(|page| page.id.clone());
            id.and_then(|id| idx.remove_page(&id))
        };
        if let Some(page) = &gone {
            self.write_text(|db| db.remove_page(&page.id));
        }
        Ok(gone)
    }

    /// Reparse every file of the space, into the index and search.db alike,
    /// and drop its vectors, which the embed task then makes again from every
    /// note once the index is written. Blocks for as long as that takes.
    /// Returns how many notes and pages it holds.
    pub fn rebuild(&self) -> Result<usize> {
        let rebuilt = {
            let mut search = lock(&self.search, "search.db")?;
            let Some(db) = search.as_mut() else {
                return Ok(0);
            };
            index::rebuild(&self.root, db)
        };
        let count = rebuilt.len();
        if let Some(mut idx) = self.index_to_change()? {
            *idx = rebuilt;
        }
        // Under the lock, so a pass under way neither stores into the old
        // vectors nor loads them before they are gone.
        let mut vectors = lock(&self.vectors, "vectors")?;
        *vectors = None;
        if let Some(db) = lock(&self.db, "space.db")?.as_mut() {
            db.clear_vectors()
                .map_err(|e| format!("could not remove the vectors: {e}"))?;
        }
        Ok(count)
    }

    /// Run `read` on the index and the text together, index first as the
    /// locks go. `None` while the space is not open.
    pub fn read<T>(
        &self,
        read: impl FnOnce(&Index, &SearchDb) -> Result<T>,
    ) -> Result<Option<T>> {
        let idx = read_lock(&self.index, "index")?;
        let search = lock(&self.search, "search.db")?;
        match (idx.is_closed(), search.as_ref()) {
            (false, Some(db)) => read(&idx, db).map(Some),
            _ => Ok(None),
        }
    }

    /// Fill in the text of `notes`, taken from the index without it.
    pub fn fill_bodies(&self, notes: &mut [Note]) {
        let ids: Vec<String> = notes.iter().map(|note| note.id.clone()).collect();
        let mut bodies = self.bodies(&ids);
        for note in notes {
            if let Some(body) = bodies.remove(&note.id) {
                note.body = body;
            }
        }
    }

    /// The text of `ids`, those search.db holds. Empty while the space is not
    /// open.
    pub fn bodies(&self, ids: &[String]) -> HashMap<String, String> {
        let Ok(search) = self.search.lock() else {
            return HashMap::new();
        };
        search
            .as_ref()
            .and_then(|db| db.bodies(ids.iter().map(String::as_str)).ok())
            .unwrap_or_default()
    }

    /// Write `index.jsonl` from what is in memory. A space that is not open
    /// holds nothing to write, and its file stays as it was.
    pub async fn persist_index(&self, writer: &Writer) -> Result<()> {
        if self.is_retired() {
            return Ok(());
        }
        let jsonl = {
            let idx = read_lock(&self.index, "index")?;
            if idx.is_closed() {
                return Ok(());
            }
            idx.to_jsonl()
        };
        // Every change to the index but a capture ends here, so the vectors
        // follow it from this one place.
        self.index_changed();
        writer
            .write(index::index_path(&self.root), jsonl)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("scratchnote-spaces-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    /// A day of one note, written as the writer would.
    fn write_note(root: &Path, id: &str, date: &str) -> Note {
        use crate::storage::daily_file::{append_note, body_hash, Kind};
        let note = Note {
            id: id.to_string(),
            date: date.to_string(),
            time: "08:00".to_string(),
            file: crate::storage::relative_day_path(date),
            subject: None,
            hash: body_hash("a note"),
            ahead_off: false,
            body: "a note".to_string(),
            kind: Kind::Note,
            on: None,
            missing: false,
        };
        let path = day_path(root, date);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, append_note("", &note, date)).unwrap();
        note
    }

    fn wake() -> Wake {
        std::sync::Arc::new(tokio::sync::Notify::new())
    }

    #[test]
    fn a_change_to_threads_is_undone_once_and_never_over_a_later_one() {
        let root = scratch("undo-threads");
        write_note(&root, "01AAA", "2026-09-22");
        let space = Space::new("Test", root.clone(), wake());
        space.load();
        assert_eq!(space.undo_threads().unwrap(), None, "nothing to undo yet");

        // A merge-like change: the edits and the pins both move.
        let before = space.decided();
        space
            .change_edits(|edits| edits.alone.insert("01AAA".into()))
            .unwrap();
        space
            .change_pins(|pins| {
                pins.push(Pin {
                    kind: pins::PinKind::Thread,
                    target: "01AAA".into(),
                    label: None,
                });
                true
            })
            .unwrap();
        space.remember_change(before.clone());
        assert_eq!(space.undo_threads().unwrap(), Some(true));
        assert_eq!(space.decided(), before);
        assert_eq!(space.undo_threads().unwrap(), None, "only once");

        // Anything changed after it keeps the older change from coming undone.
        let before = space.decided();
        space
            .change_edits(|edits| {
                edits.titles.insert("01AAA".into(), "Kitchen".into());
                true
            })
            .unwrap();
        space.remember_change(before);
        space
            .change_edits(|edits| edits.alone.insert("01AAA".into()))
            .unwrap();
        assert_eq!(space.undo_threads().unwrap(), None);
        assert!(space.edits().titles.contains_key("01AAA"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_space_is_read_only_while_it_is_open() {
        let root = scratch("open-close");
        write_note(&root, "01AAA", "2026-09-22");

        let space = Space::new("Test", root.clone(), wake());
        assert!(!space.is_open());
        assert_eq!(space.note_count(), None);

        space.load();
        assert!(space.is_open());
        assert_eq!(space.note_count(), Some(1));

        assert_eq!(space.unload(), Some(1));
        assert!(!space.is_open());
        assert_eq!(space.note_count(), None);
        assert_eq!(space.unload(), None, "no count to record twice");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_closed_space_drops_changes_and_leaves_its_cache_alone() {
        let root = scratch("closed-writes");
        write_note(&root, "01AAA", "2026-09-22");
        let writer = Writer::spawn();
        let (space, _) = Space::open("Test", root.clone(), wake());
        space.persist_index(&writer).await.unwrap();
        let cache = index::index_path(&root);
        let before = std::fs::read_to_string(&cache).unwrap();
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
        filetime::set_file_mtime(&cache, filetime::FileTime::from_system_time(old)).unwrap();

        // What a job or a late capture does once the space is left.
        space.unload();
        write_note(&root, "01BBB", "2026-09-23");
        space.day_changed("2026-09-23").unwrap();
        let late = write_note(&root, "01CCC", "2026-09-24");
        space.note_added(&writer, &late).await.unwrap();
        space.persist_index(&writer).await.unwrap();
        assert_eq!(std::fs::read_to_string(&cache).unwrap(), before);

        // Opened again, the days written meanwhile are newer than the cache.
        space.load();
        assert_eq!(space.note_count(), Some(3));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_rebuild_drops_the_vectors_to_embed_every_note_again() {
        let root = scratch("rebuild-vectors");
        write_note(&root, "01AAA", "2026-09-22");
        let (space, _) = Space::open("Test", root.clone(), wake());
        let mut vectors = Vectors::new("m", 2);
        vectors
            .insert("01AAA".into(), "h".into(), vec![1.0, 0.0])
            .unwrap();
        vectors
            .save(space.db.lock().unwrap().as_mut().unwrap())
            .unwrap();
        *space.vectors.lock().unwrap() = Some(vectors);

        assert_eq!(space.rebuild().unwrap(), 1);
        assert!(space.vectors.lock().unwrap().is_none());
        let saved = Vectors::load(space.db.lock().unwrap().as_ref().unwrap(), "m", 2);
        assert_eq!(saved.len(), 0);
        drop(space);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_file_opens_the_first_space() {
        let registry = Registry::load(&scratch("missing"));
        assert_eq!(registry.active, FIRST_NAME);
    }

    #[test]
    fn the_space_at_the_root_moves_into_its_own_folder() {
        let root = scratch("migrate");
        std::fs::create_dir_all(root.join("notes").join("2026")).unwrap();
        std::fs::write(root.join("notes/2026/2026-09-22.md"), "x").unwrap();
        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
        std::fs::write(index::index_path(&root), "").unwrap();
        std::fs::write(
            registry_path(&root),
            r#"{"active":"Home","defaultName":"Home"}"#,
        )
        .unwrap();
        // Already taken, ignoring case, so the moved space gets another name.
        std::fs::create_dir_all(spaces_dir(&root).join("home")).unwrap();

        migrate_root(&root);

        let to = spaces_dir(&root).join("Home 2");
        assert!(to.join("notes/2026/2026-09-22.md").exists());
        assert!(index::index_path(&to).exists());
        assert!(!root.join("notes").exists());
        assert!(!index::index_path(&root).exists());
        assert_eq!(Registry::load(&root).active, "Home 2");

        // Nothing left at the root, so a second launch changes nothing.
        migrate_root(&root);
        assert!(to.join("notes/2026/2026-09-22.md").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn names_are_tidied_and_must_make_a_folder() {
        assert_eq!(check_name("  Work   stuff ").unwrap(), "Work stuff");
        assert_eq!(check_name("Journal 2026").unwrap(), "Journal 2026");
        assert_eq!(check_name("日記").unwrap(), "日記");
        for bad in [
            "",
            "   ",
            "a/b",
            "a\\b",
            "what?",
            "C:",
            "..",
            ".hidden",
            "trailing.",
            "CON",
            "com1",
            "nul.txt",
        ] {
            assert!(check_name(bad).is_err(), "{bad:?} should be refused");
        }
        assert!(check_name("COM").is_ok());
        assert!(check_name("Console").is_ok());
        assert!(check_name(&"x".repeat(MAX_NAME + 1)).is_err());
    }

    #[test]
    fn every_usable_folder_under_spaces_is_a_space() {
        let root = scratch("discover");
        for dir in ["work", "Books", "two  spaces", ".git"] {
            std::fs::create_dir_all(spaces_dir(&root).join(dir)).unwrap();
        }
        std::fs::write(spaces_dir(&root).join("stray.md"), "x").unwrap();
        let names: Vec<String> = discover(&root).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, vec!["Books", "work"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
