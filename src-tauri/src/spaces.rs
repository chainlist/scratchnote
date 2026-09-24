//! Spaces: separate sets of notes, each with its own tags, categories, index
//! and queue.
//!
//! The notes root is the default space, so an install from before spaces
//! existed keeps working untouched. Every other space is a folder of the same
//! shape under `spaces/`, named after the space, so a folder made there by
//! hand is a space too. `.scratchnote/spaces.json` at the root records only
//! what the folders cannot: the default space's name and which space is open.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock};

use serde::{Deserialize, Serialize};

use crate::enrich::normalize::Vocabulary;
use crate::enrich::queue::{queue_path, Job, Queue};
use crate::storage::index::{self, Index};
use crate::storage::writer::Writer;
use crate::storage::{categories, tags};

const DEFAULT_NAME: &str = "Personal";
/// Long enough for any sensible name, short enough for the sidebar.
const MAX_NAME: usize = 40;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Registry {
    /// Name of the open space.
    pub active: String,
    /// The default space has no folder of its own to be named after.
    pub default_name: String,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            active: DEFAULT_NAME.to_string(),
            default_name: DEFAULT_NAME.to_string(),
        }
    }
}

impl Registry {
    /// A missing or broken file opens the default space.
    pub fn load(root: &Path) -> Self {
        let mut registry = match std::fs::read_to_string(registry_path(root)) {
            Ok(raw) => serde_json::from_str::<Registry>(&raw).unwrap_or_else(|e| {
                log::warn!("spaces.json is not readable ({e}), opening the default space");
                Registry::default()
            }),
            Err(_) => Registry::default(),
        };
        // Hand-edited to something that is not a usable name.
        match check_name(&registry.default_name) {
            Ok(name) => registry.default_name = name,
            Err(_) => registry.default_name = DEFAULT_NAME.to_string(),
        }
        registry
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

/// One space as loaded: its cache, vocabulary and queue.
pub struct Space {
    pub name: String,
    /// The default space lives at the notes root rather than under `spaces/`.
    pub is_default: bool,
    pub root: PathBuf,
    /// The derived cache from SPEC 4.4. Guards are held only for the length of
    /// a read or a swap, never across an await.
    pub index: RwLock<Index>,
    /// The user's half of `tags.json` (SPEC 4.5), read at startup and again on
    /// a rebuild.
    pub aliases: RwLock<HashMap<String, String>>,
    /// Pending enrichment jobs (SPEC 5.6).
    pub queue: Mutex<Queue>,
    /// Held only to keep it alive; dropping it stops watching.
    pub watcher: Mutex<Option<notify::RecommendedWatcher>>,
    /// Set once the space is deleted or renamed away, so a job still in hand
    /// for it does not write files back into the old folder.
    pub retired: AtomicBool,
}

impl Space {
    /// Load a space from disk. The flag says the index on disk is out of date
    /// and should be written back.
    pub fn open(name: &str, is_default: bool, root: PathBuf) -> (Self, bool) {
        let (loaded, stale) = index::load(&root);
        categories::ensure(&root);
        // tags.json is written alongside the index, so a missing one means
        // the counts were never written, not that there are no tags.
        let refresh = stale || !tags::tags_path(&root).exists();
        log::info!("space {name} holds {} notes", loaded.len());

        // The queue survives restarts, and anything still pending in the
        // markdown is re-queued in case the queue file was lost.
        let mut queue = Queue::load(&root);
        for (note, date) in loaded.pending() {
            queue.push(Job::new(note, date));
        }

        let space = Self {
            name: name.to_string(),
            is_default,
            aliases: RwLock::new(tags::load_aliases(&root)),
            index: RwLock::new(loaded),
            queue: Mutex::new(queue),
            watcher: Mutex::new(None),
            retired: AtomicBool::new(false),
            root,
        };
        (space, refresh)
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

    pub fn queued(&self) -> usize {
        self.queue.lock().map(|queue| queue.len()).unwrap_or(0)
    }

    pub fn note_count(&self) -> usize {
        self.index.read().map(|idx| idx.len()).unwrap_or(0)
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
    pub async fn persist_index(&self, writer: &Writer) -> Result<(), String> {
        if self.is_retired() {
            return Ok(());
        }
        let (jsonl, counts) = {
            let idx = self
                .index
                .read()
                .map_err(|_| "index lock poisoned".to_string())?;
            (idx.to_jsonl(), idx.tag_counts())
        };
        writer
            .write_index(index::index_path(&self.root), jsonl)
            .await?;
        writer
            .write_index(
                tags::tags_path(&self.root),
                tags::render(&counts, &self.aliases()),
            )
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

    #[test]
    fn a_missing_file_opens_the_default_space() {
        let registry = Registry::load(&scratch("missing"));
        assert_eq!(registry.active, DEFAULT_NAME);
        assert_eq!(registry.default_name, DEFAULT_NAME);
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
