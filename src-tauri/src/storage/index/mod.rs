//! `index.jsonl`, the derived cache from SPEC 4.4.
//!
//! Nothing here is authoritative. Every entry can be reproduced by reparsing
//! the markdown, which is what a rebuild does and what startup does for any
//! day or page whose file changed since it was last read. Deleting
//! `.scratchnote/` costs nothing but the time to reparse. The notes' text is
//! not held here but in `search.db` (SPEC 6), which follows the same files.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::daily_file::{self, Kind, Note};
use super::paths::meta_dir;
use super::search_db::{SearchDb, Stamp};
use super::{check_date, page_file, relative_day_path};

/// One note or page, without its text: that is in `search.db` (SPEC 6), so
/// this stays small in memory as in `index.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    pub id: String,
    pub date: String,
    pub time: String,
    pub file: String,
    pub subject: Option<String>,
    pub hash: String,
    /// The later day the note looks forward to (SPEC 5.3), which brings it
    /// back on that day's view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
    /// A page's `file` is its own and its `subject` its title (SPEC 4.7).
    #[serde(default, skip_serializing_if = "Kind::is_note")]
    pub kind: Kind,
    /// How many words the body holds, counted once here so the day list
    /// does not count every body again. `None` only on a line from a cache
    /// written before words were counted, whose day `load` reparses.
    #[serde(default)]
    pub words: Option<usize>,
}

impl From<&Note> for IndexEntry {
    fn from(note: &Note) -> Self {
        Self {
            id: note.id.clone(),
            date: note.date.clone(),
            time: note.time.clone(),
            file: note.file.clone(),
            subject: note.subject.clone(),
            hash: note.hash.clone(),
            on: note.on.clone(),
            kind: note.kind,
            words: Some(words(&note.body)),
        }
    }
}

impl IndexEntry {
    /// As a note without its text, which `search::with_bodies` reads.
    pub fn to_note(&self) -> Note {
        Note {
            id: self.id.clone(),
            date: self.date.clone(),
            time: self.time.clone(),
            file: self.file.clone(),
            subject: self.subject.clone(),
            hash: self.hash.clone(),
            on: self.on.clone(),
            ahead_off: false,
            body: String::new(),
            kind: self.kind,
            missing: false,
        }
    }

    /// Newest first: by date, then by time within the day.
    pub fn newest_first(a: &IndexEntry, b: &IndexEntry) -> Ordering {
        b.date.cmp(&a.date).then_with(|| b.time.cmp(&a.time))
    }
}

/// Notes grouped by day, so one file's worth can be replaced wholesale when
/// that file changes, and pages by id, since each has a file of its own.
#[derive(Debug, Default)]
pub struct Index {
    by_date: BTreeMap<String, Vec<IndexEntry>>,
    pages: BTreeMap<String, IndexEntry>,
    /// Set on what a space holds while it is not open (SPEC 4.6). Kept with
    /// the notes, under their lock, so a write that races the space closing
    /// is dropped rather than saving an empty cache over the real one.
    closed: bool,
}

impl Index {
    /// What a space holds while it is not open: no notes, and marked so.
    pub fn closed() -> Self {
        Self {
            closed: true,
            ..Self::default()
        }
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// A day's notes. Its pages come from their own files, so they stay.
    /// Returns whether that changed what the day held.
    pub fn replace_day(&mut self, date: &str, entries: Vec<IndexEntry>) -> bool {
        let same = self
            .by_date
            .get(date)
            .map_or(entries.is_empty(), |held| *held == entries);
        if entries.is_empty() {
            self.by_date.remove(date);
        } else {
            self.by_date.insert(date.to_string(), entries);
        }
        !same
    }

    /// Add a note to its day, or a page by its id, replacing a page of that
    /// id: notes are kept by day, pages each on their own.
    pub fn push(&mut self, entry: IndexEntry) {
        if entry.kind == Kind::Page {
            self.pages.insert(entry.id.clone(), entry);
            return;
        }
        self.by_date
            .entry(entry.date.clone())
            .or_default()
            .push(entry);
    }

    /// Add a page, or replace it when its id is already there. Returns the
    /// entry it replaced.
    pub fn replace_page(&mut self, entry: IndexEntry) -> Option<IndexEntry> {
        self.pages.insert(entry.id.clone(), entry)
    }

    pub fn remove_page(&mut self, id: &str) -> Option<IndexEntry> {
        self.pages.remove(id)
    }

    pub fn page(&self, id: &str) -> Option<&IndexEntry> {
        self.pages.get(id)
    }

    /// The page held in `file`, a path relative to the space's root.
    pub fn page_at(&self, file: &str) -> Option<&IndexEntry> {
        self.pages.values().find(|page| page.file == file)
    }

    pub fn pages(&self) -> impl Iterator<Item = &IndexEntry> {
        self.pages.values()
    }

    /// The pages whose day is `date`.
    pub fn pages_on<'a>(&'a self, date: &'a str) -> impl Iterator<Item = &'a IndexEntry> {
        self.pages.values().filter(move |page| page.date == date)
    }

    /// Forget days that no longer have a file on disk.
    pub fn retain_days(&mut self, present: &HashSet<String>) -> bool {
        let before = self.by_date.len();
        self.by_date.retain(|date, _| present.contains(date));
        self.by_date.len() != before
    }

    /// Dates newest first, with how many notes and pages each holds.
    pub fn days(&self) -> Vec<(String, usize)> {
        let mut counts: BTreeMap<&str, usize> = self
            .by_date
            .iter()
            .map(|(date, entries)| (date.as_str(), entries.len()))
            .collect();
        for page in self.pages.values() {
            *counts.entry(page.date.as_str()).or_insert(0) += 1;
        }
        counts
            .into_iter()
            .rev()
            .map(|(date, count)| (date.to_string(), count))
            .collect()
    }

    /// How many words the notes of `date` hold, its pages left out.
    pub fn words_on(&self, date: &str) -> usize {
        self.by_date.get(date).map_or(0, |entries| {
            entries.iter().filter_map(|entry| entry.words).sum()
        })
    }

    /// Whether a note of `date` came from a cache that did not count words.
    fn uncounted(&self, date: &str) -> bool {
        self.by_date
            .get(date)
            .is_some_and(|entries| entries.iter().any(|entry| entry.words.is_none()))
    }

    pub fn len(&self) -> usize {
        self.by_date.values().map(Vec::len).sum::<usize>() + self.pages.len()
    }

    /// Notes day by day, then pages.
    pub fn entries(&self) -> impl Iterator<Item = &IndexEntry> {
        self.by_date.values().flatten().chain(self.pages.values())
    }

    /// The note or page `id`, the first `entries` holds.
    pub fn get(&self, id: &str) -> Option<&IndexEntry> {
        self.entries().find(|entry| entry.id == id)
    }

    /// Every entry by id, for looking up many. An id held twice, as by a
    /// note copied by hand into another day, gives the last.
    pub fn by_id(&self) -> HashMap<&str, &IndexEntry> {
        self.entries()
            .map(|entry| (entry.id.as_str(), entry))
            .collect()
    }

    /// The entries of `ids` the index holds, in the order of `ids`.
    pub fn pick<'a>(&self, ids: impl IntoIterator<Item = &'a str>) -> Vec<&IndexEntry> {
        let entries = self.by_id();
        ids.into_iter()
            .filter_map(|id| entries.get(id).copied())
            .collect()
    }

    pub fn to_jsonl(&self) -> String {
        let mut out = String::new();
        for entry in self.entries() {
            match serde_json::to_string(entry) {
                Ok(line) => {
                    out.push_str(&line);
                    out.push('\n');
                }
                Err(e) => log::warn!("could not serialise index entry {}: {e}", entry.id),
            }
        }
        out
    }

    /// An unreadable line costs that one note until the next rebuild, rather
    /// than the whole cache.
    pub fn from_jsonl(raw: &str) -> Self {
        let mut index = Self::default();
        for line in raw.lines().filter(|l| !l.trim().is_empty()) {
            match serde_json::from_str::<IndexEntry>(line) {
                Ok(entry) => index.push(entry),
                Err(e) => log::warn!("skipping unreadable index line: {e}"),
            }
        }
        index
    }
}

pub fn index_path(root: &Path) -> PathBuf {
    meta_dir(root).join("index.jsonl")
}

/// Every `notes/<year>/<date>.md` under the root, with the date it holds.
pub fn daily_files(root: &Path) -> Vec<(String, PathBuf)> {
    let mut found = Vec::new();
    let Ok(years) = std::fs::read_dir(root.join("notes")) else {
        return found;
    };
    for year in years.flatten() {
        let Ok(files) = std::fs::read_dir(year.path()) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Some(date) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if check_date(date).is_err() {
                continue;
            }
            found.push((date.to_string(), path));
        }
    }
    found.sort();
    found
}

/// Parse one day's file. A missing or unreadable file yields no entries, which
/// is also how a day that was deleted externally drops out of the index.
pub fn parse_day(path: &Path, date: &str) -> Vec<Note> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    daily_file::parse_notes(&contents, date, &relative_day_path(date))
}

/// What the index keeps of a day's notes.
pub fn entries(notes: &[Note]) -> Vec<IndexEntry> {
    notes.iter().map(IndexEntry::from).collect()
}

/// Every markdown file under `pages/`, at any depth, since a page moved by
/// hand is still found by its id. Folders starting with a dot are skipped.
pub fn page_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut dirs = vec![root.join("pages")];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let hidden = entry.file_name().to_string_lossy().starts_with('.');
            match entry.file_type() {
                Ok(t) if t.is_dir() && !hidden => dirs.push(path),
                Ok(t) if t.is_file() && path.extension().and_then(|e| e.to_str()) == Some("md") => {
                    found.push(path)
                }
                _ => {}
            }
        }
    }
    found.sort();
    found
}

/// A path under the root as the index writes it: relative, with forward
/// slashes on every platform.
pub fn relative(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

/// Parse one page file. `None` for a file that is missing, unreadable or not
/// a page.
pub fn parse_page(root: &Path, path: &Path) -> Option<Note> {
    let contents = std::fs::read_to_string(path).ok()?;
    page_file::parse_page(&contents, &relative(root, path)?)
}

/// Bring the pages of the index and of `db` in step with the files under
/// `pages/`. A file is read when `read`, what `db` recorded of the files, has
/// it at another time or length, or when it is newer than the cache, written
/// at `cached_at`; any other is not read at all. Returns whether the index
/// changed.
fn read_pages(
    root: &Path,
    db: &mut SearchDb,
    index: &mut Index,
    read: &HashMap<String, Stamp>,
    cached_at: i64,
) -> bool {
    let files: Vec<(String, PathBuf)> = page_files(root)
        .into_iter()
        .filter_map(|path| Some((relative(root, &path)?, path)))
        .collect();
    let present: HashSet<&str> = files.iter().map(|(file, _)| file.as_str()).collect();
    let mut changed = false;

    // A page whose file is gone leaves first, so one renamed by hand is
    // found at its new name rather than taken for a copy of itself.
    let gone: Vec<String> = index
        .pages()
        .filter(|page| !present.contains(page.file.as_str()))
        .map(|page| page.id.clone())
        .collect();
    for id in &gone {
        index.remove_page(id);
        changed = true;
    }

    for (file, path) in &files {
        // Taken before the file is read, as for a day.
        let stamp = Stamp::of(path);
        let newer = stamp.is_some_and(|stamp| stamp.modified > cached_at);
        let stale_text = stamp.is_none() || read.get(file) != stamp.as_ref();
        if !newer && !stale_text {
            continue;
        }
        changed |= newer;
        let page = parse_page(root, path);
        // The page the file held, if it now holds another or none, goes.
        let held = index.page_at(file).map(|held| held.id.clone());
        if let Some(held) = held.filter(|held| page.as_ref().map(|page| &page.id) != Some(held)) {
            index.remove_page(&held);
            changed = true;
        }
        let written = match page {
            // A copy made by hand of a page held in another file is left
            // out, and not recorded as read, so it is taken once that file
            // goes.
            Some(page)
                if index
                    .page(&page.id)
                    .is_some_and(|other| other.file != *file) =>
            {
                log::warn!("{} repeats page {}; skipping it", path.display(), page.id);
                db.stamp_page_file(file, None)
            }
            Some(page) => {
                let entry = IndexEntry::from(&page);
                changed |= !index
                    .replace_page(entry.clone())
                    .is_some_and(|before| before == entry);
                db.replace_page(&page, stamp)
            }
            None => db.stamp_page_file(file, stamp),
        };
        if let Err(e) = written {
            log::warn!("could not put {file} in search.db: {e}");
        }
    }

    let ids: HashSet<&str> = index.pages().map(|page| page.id.as_str()).collect();
    if let Err(e) = db.retain_pages(&present, &ids) {
        log::warn!("could not drop the pages gone from search.db: {e}");
    }
    changed
}

/// The runs of text between spaces that hold a letter or a digit, so a
/// list's `-` or a task's box is not a word.
fn words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|token| !matches!(*token, "[x]" | "[X]"))
        .filter(|token| token.chars().any(char::is_alphanumeric))
        .count()
}

/// Reparse every daily file and page file under the root, into the index
/// and into `db`, which is emptied first.
pub fn rebuild(root: &Path, db: &mut SearchDb) -> Index {
    let mut index = Index::default();
    if let Err(e) = db.clear() {
        log::warn!("could not empty search.db: {e}");
    }
    db.in_bulk(|db| {
        for (date, path) in daily_files(root) {
            let stamp = Stamp::of(&path);
            let notes = parse_day(&path, &date);
            if let Err(e) = db.replace_day(&date, &notes, stamp) {
                log::warn!("could not put {date} in search.db: {e}");
            }
            index.replace_day(&date, entries(&notes));
        }
        read_pages(root, db, &mut index, &HashMap::new(), i64::MIN);
    });
    index
}

/// Load the cache, reparsing only the days whose file changed since it was
/// last read, and dropping days whose file is gone. What a reparse finds goes
/// into `db` too. The flag says whether the index differs from what is on
/// disk, so the caller knows if it needs writing back.
pub fn load(root: &Path, db: &mut SearchDb) -> (Index, bool) {
    let path = index_path(root);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return (rebuild(root, db), true);
    };

    // A cache from before the model was dropped holds the labels it wrote,
    // and the days ahead it read.
    if raw.contains("\"status\":") {
        return (rebuild(root, db), true);
    }

    let cached_at = Stamp::of(&path).map_or(i64::MIN, |stamp| stamp.modified);
    let mut index = Index::from_jsonl(&raw);
    let files = daily_files(root);
    let read = db.days().unwrap_or_else(|e| {
        log::warn!("could not read search.db, reading every day again: {e}");
        HashMap::new()
    });
    let pages_read = db.page_files().unwrap_or_else(|e| {
        log::warn!("could not read search.db, reading every page again: {e}");
        HashMap::new()
    });
    let mut changed = false;

    let read_again = |db: &mut SearchDb| {
        // A day is read again when its file is newer than the cache, when the
        // cache did not count its words, or when search.db last read it at
        // another time or length. Any other day is not read at all.
        for (date, file) in &files {
            let stamp = Stamp::of(file);
            let newer = stamp.is_some_and(|stamp| stamp.modified > cached_at);
            let stale_meta = newer || index.uncounted(date);
            let stale_text = stamp.is_none() || read.get(date) != stamp.as_ref();
            if !stale_meta && !stale_text {
                continue;
            }
            let notes = parse_day(file, date);
            if let Err(e) = db.replace_day(date, &notes, stamp) {
                log::warn!("could not put {date} in search.db: {e}");
            }
            // Read again for search.db alone, a day can still say something
            // the cache did not, and then the cache is written back too.
            let differs = index.replace_day(date, entries(&notes));
            changed |= stale_meta || differs;
        }

        let present: HashSet<String> = files.iter().map(|(date, _)| date.clone()).collect();
        changed |= index.retain_days(&present);
        if let Err(e) = db.retain_days(&present) {
            log::warn!("could not drop the days gone from search.db: {e}");
        }

        changed |= read_pages(root, db, &mut index, &pages_read, cached_at);
    };
    // A search.db that has read no file yet, new or from another version, is
    // filled in one go.
    if read.is_empty() && pages_read.is_empty() {
        db.in_bulk(read_again);
    } else {
        db.at_once(read_again);
    }

    (index, changed)
}

#[cfg(test)]
mod tests;
