//! Spaces: separate sets of notes, each with its own categories, index and
//! queue.
//!
//! Every space is a folder under `spaces/`, named after the space, so a folder
//! made there by hand is a space too. `.scratchnote/spaces.json` at the root
//! records only what the folders cannot: which space is open.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock, RwLockWriteGuard};

use serde::{Deserialize, Serialize};

use crate::embed::vectors::Vectors;
use crate::enrich::queue::{queue_path, Job, Queue};
use crate::enrich::worker::Wake;
use crate::storage::day_path;
use crate::storage::index::{self, Index, IndexEntry};
use crate::storage::writer::Writer;
use crate::storage::categories;

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
    root.join(".scratchnote").join("spaces.json")
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
    let name = std::iter::once(wanted.clone())
        .chain((2..).map(|n| format!("{wanted} {n}")))
        .find(|n| !taken.contains(&n.to_lowercase()))
        .unwrap_or(wanted);
    let to = spaces_dir(root).join(&name);

    let moved = std::fs::create_dir_all(to.join(".scratchnote"))
        .and_then(|_| std::fs::rename(&notes, to.join("notes")));
    if let Err(e) = moved {
        log::error!(
            "could not move {} into {}: {e}",
            notes.display(),
            to.display()
        );
        return;
    }
    let own: [fn(&Path) -> PathBuf; 3] = [
        index::index_path,
        queue_path,
        categories::categories_path,
    ];
    for path in own {
        let from = path(root);
        if from.exists() {
            if let Err(e) = std::fs::rename(&from, path(&to)) {
                log::warn!(
                    "could not move {} into {}: {e}",
                    from.display(),
                    to.display()
                );
            }
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
pub fn check_name(raw: &str) -> Result<String, String> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err("a space needs a name".to_string());
    }
    if name.chars().count() > MAX_NAME {
        return Err(format!("keep the name under {MAX_NAME} characters"));
    }
    if let Some(c) = name
        .chars()
        .find(|c| c.is_control() || r#"<>:"/\|?*"#.contains(*c))
    {
        return Err(format!("a space name cannot contain {c}"));
    }
    if name.starts_with('.') || name.ends_with('.') {
        return Err("a space name cannot start or end with a dot".to_string());
    }
    // Windows refuses these as file names, with or without an extension.
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    if reserved {
        return Err(format!("{name} is a name Windows keeps for itself"));
    }
    Ok(name)
}

/// One space: its queue always, and its cache while it is open. Only the
/// open space is read into memory and watched (SPEC 4.6); the others keep
/// their queue, which the worker goes on with.
pub struct Space {
    pub name: String,
    pub root: PathBuf,
    /// The derived cache from SPEC 4.4, or `Index::closed()` while the space
    /// is not open. Guards are held only for the length of a read or a swap,
    /// never across an await.
    pub index: RwLock<Index>,
    /// Pending enrichment jobs (SPEC 5.6).
    pub queue: Mutex<Queue>,
    /// The note embeddings the chat searches. `None` until the embed task
    /// first runs with a model, which loads them from `vectors.bin`.
    pub vectors: Mutex<Option<Vectors>>,
    /// Nudges the embed task whenever the index changes.
    embed_wake: Wake,
    /// Pages open in the editor. Neither model touches them until the page
    /// view closes, so autosaves do not run either one again (SPEC 3.5).
    editing: Mutex<HashSet<String>>,
    /// Held only to keep it alive; dropping it stops watching.
    pub watcher: Mutex<Option<notify::RecommendedWatcher>>,
    /// Set once the space is deleted or renamed away, so a job still in hand
    /// for it does not write files back into the old folder.
    pub retired: AtomicBool,
}

impl Space {
    /// A space as listed, not open: only its queue is read, which survives
    /// restarts.
    pub fn new(name: &str, root: PathBuf, embed_wake: Wake) -> Self {
        Self {
            name: name.to_string(),
            index: RwLock::new(Index::closed()),
            queue: Mutex::new(Queue::load(&root)),
            vectors: Mutex::new(None),
            watcher: Mutex::new(None),
            retired: AtomicBool::new(false),
            root,
            embed_wake,
            editing: Mutex::new(HashSet::new()),
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
        let (loaded, stale) = index::load(&self.root);
        categories::ensure(&self.root);
        log::info!("space {} holds {} notes", self.name, loaded.len());

        // Anything still pending in the markdown is queued again, in case the
        // queue file was lost.
        if let Ok(mut queue) = self.queue.lock() {
            for (note, date) in loaded.pending() {
                queue.push(Job::new(note, date));
            }
        }
        if let Ok(mut index) = self.index.write() {
            *index = loaded;
        }
        // Its notes may have been written with no model, or by another one.
        self.embed_wake.notify_one();
        stale
    }

    /// Let go of the notes, the vectors and the watcher, as leaving the space
    /// does. The queue stays for the worker. Returns how many notes the space
    /// held, for the switcher to show meanwhile, and `None` if it was not
    /// open.
    pub fn unload(&self) -> Option<usize> {
        let count = self.index.write().ok().and_then(|mut index| {
            let held = std::mem::replace(&mut *index, Index::closed());
            (!held.is_closed()).then(|| held.len())
        });
        if let Ok(mut vectors) = self.vectors.lock() {
            *vectors = None;
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
    }

    pub fn is_retired(&self) -> bool {
        self.retired.load(Ordering::SeqCst)
    }

    /// A page is being edited: the models leave it alone until `release`.
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

    pub fn is_held(&self, id: &str) -> bool {
        self.editing
            .lock()
            .is_ok_and(|editing| editing.contains(id))
    }

    /// The pages being edited, for the embed task to skip.
    pub fn held(&self) -> HashSet<String> {
        self.editing
            .lock()
            .map(|editing| editing.clone())
            .unwrap_or_default()
    }

    pub fn queued(&self) -> usize {
        self.queue.lock().map(|queue| queue.len()).unwrap_or(0)
    }

    /// `None` while the space is not open, which holds no notes to count.
    pub fn note_count(&self) -> Option<usize> {
        self.index
            .read()
            .ok()
            .filter(|idx| !idx.is_closed())
            .map(|idx| idx.len())
    }

    /// The categories the model picks from, in whatever language it labels.
    pub fn categories(&self) -> Vec<String> {
        categories::load(&self.root)
    }

    /// The `k` notes closest to a query, best first, with their cosine. Empty
    /// until the embed task has loaded the vectors.
    pub fn nearest(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        self.vectors
            .lock()
            .ok()
            .and_then(|vectors| Some(vectors.as_ref()?.top(query, k)))
            .unwrap_or_default()
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

    /// Tell the embed task the index changed. `persist_index` does it, and
    /// so does `note_added`, which appends to the index instead.
    pub fn index_changed(&self) {
        self.embed_wake.notify_one();
    }

    // The index is changed through the methods below only, so whatever has
    // to follow it does so from one place. Each takes the lock for its own
    // change; `persist_index` then writes the lot back once. A space that is
    // not open drops the change: its files are then newer than its cache, so
    // `index::load` reads them again when it opens.

    /// The index to change, or `None` while the space is not open.
    fn index_to_change(&self) -> Result<Option<RwLockWriteGuard<'_, Index>>, String> {
        let idx = self
            .index
            .write()
            .map_err(|_| "index lock poisoned".to_string())?;
        Ok((!idx.is_closed()).then_some(idx))
    }

    /// A note just captured: pushed rather than its day reparsed, and its
    /// line appended to `index.jsonl` rather than the file rewritten, since
    /// capture has a latency budget.
    pub async fn note_added(&self, writer: &Writer, entry: IndexEntry) -> Result<(), String> {
        let line = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
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
    pub fn day_changed(&self, date: &str) -> Result<(), String> {
        if !self.is_open() {
            return Ok(());
        }
        self.set_day(date, index::parse_day(&day_path(&self.root, date), date))
    }

    /// Put `entries`, parsed from a day's file, in place of that day's notes.
    pub fn set_day(&self, date: &str, entries: Vec<IndexEntry>) -> Result<(), String> {
        if let Some(mut idx) = self.index_to_change()? {
            idx.replace_day(date, entries);
        }
        Ok(())
    }

    /// A page added or changed. Returns what the index had for it before.
    pub fn page_changed(&self, entry: IndexEntry) -> Result<Option<IndexEntry>, String> {
        Ok(self
            .index_to_change()?
            .and_then(|mut idx| idx.replace_page(entry)))
    }

    /// A page deleted. Returns what the index had for it.
    pub fn page_removed(&self, id: &str) -> Result<Option<IndexEntry>, String> {
        Ok(self
            .index_to_change()?
            .and_then(|mut idx| idx.remove_page(id)))
    }

    /// Whatever page the index had at `file`, a path relative to the root,
    /// is gone from there. Returns its entry.
    pub fn page_file_gone(&self, file: &str) -> Result<Option<IndexEntry>, String> {
        let Some(mut idx) = self.index_to_change()? else {
            return Ok(None);
        };
        let id = idx.page_at(file).map(|page| page.id.clone());
        Ok(id.and_then(|id| idx.remove_page(&id)))
    }

    /// The whole index, rebuilt from the markdown. Returns how many notes and
    /// pages it holds.
    pub fn index_rebuilt(&self, rebuilt: Index) -> Result<usize, String> {
        let count = rebuilt.len();
        if let Some(mut idx) = self.index_to_change()? {
            *idx = rebuilt;
        }
        Ok(count)
    }

    /// Write `index.jsonl` from what is in memory. A space that is not open
    /// holds nothing to write, and its file stays as it was.
    pub async fn persist_index(&self, writer: &Writer) -> Result<(), String> {
        if self.is_retired() {
            return Ok(());
        }
        let jsonl = {
            let idx = self
                .index
                .read()
                .map_err(|_| "index lock poisoned".to_string())?;
            if idx.is_closed() {
                return Ok(());
            }
            idx.to_jsonl()
        };
        // Every change to the index but a capture ends here, so the vectors
        // follow it from this one place.
        self.index_changed();
        writer
            .write_index(index::index_path(&self.root), jsonl)
            .await
    }

    pub async fn persist_queue(&self, writer: &Writer) {
        if self.is_retired() {
            return;
        }
        let contents = match self.queue.lock() {
            Ok(queue) => queue.to_json(),
            Err(_) => return,
        };
        if let Err(e) = writer.write_index(queue_path(&self.root), contents).await {
            log::warn!("could not persist the queue of {}: {e}", self.name);
        }
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
    fn write_note(root: &Path, id: &str, date: &str) -> IndexEntry {
        use crate::storage::daily_file::{append_note, body_hash, Kind, Note, Status};
        let note = Note {
            id: id.to_string(),
            date: date.to_string(),
            time: "08:00".to_string(),
            file: crate::storage::relative_day_path(date),
            subject: None,
            category: None,
            status: Status::Done,
            hash: body_hash("a note"),
            lang: None,
            body: "a note".to_string(),
            kind: Kind::Note,
            missing: false,
        };
        let path = day_path(root, date);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, append_note("", &note, date)).unwrap();
        IndexEntry::from(&note)
    }

    fn wake() -> Wake {
        std::sync::Arc::new(tokio::sync::Notify::new())
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
        space.note_added(&writer, late).await.unwrap();
        space.persist_index(&writer).await.unwrap();
        assert_eq!(std::fs::read_to_string(&cache).unwrap(), before);

        // Opened again, the days written meanwhile are newer than the cache.
        space.load();
        assert_eq!(space.note_count(), Some(3));
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
        std::fs::write(categories::categories_path(&root), "[]").unwrap();
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
        assert!(categories::categories_path(&to).exists());
        assert!(!root.join("notes").exists());
        assert!(!categories::categories_path(&root).exists());
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
