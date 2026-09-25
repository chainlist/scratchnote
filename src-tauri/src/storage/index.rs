//! `index.jsonl`, the derived cache from SPEC 4.4.
//!
//! Nothing here is authoritative. Every entry can be reproduced by reparsing
//! the markdown, which is what a rebuild does and what startup does for any
//! day whose file is newer than the cache. Deleting `.scratchnote/` costs
//! nothing but the time to reparse.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::daily_file::{self, Note, Status};
use super::{check_date, relative_day_path};
use crate::enrich::language;

/// One note. The body is held in memory for search (SPEC 6) but never written
/// to `index.jsonl`, which stays a small metadata cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    pub id: String,
    pub date: String,
    pub time: String,
    pub file: String,
    pub subject: Option<String>,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub status: Status,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(skip)]
    pub body: String,
    /// Subject, summary and body folded for matching, computed once here so a
    /// search does not fold ten thousand bodies per keystroke.
    #[serde(skip)]
    pub folded: String,
}

impl From<&Note> for IndexEntry {
    fn from(note: &Note) -> Self {
        let folded = crate::search::fold(&format!(
            "{}
{}
{}",
            note.subject.as_deref().unwrap_or(""),
            note.summary.as_deref().unwrap_or(""),
            note.body
        ));
        Self {
            id: note.id.clone(),
            date: note.date.clone(),
            time: note.time.clone(),
            file: note.file.clone(),
            subject: note.subject.clone(),
            summary: note.summary.clone(),
            tags: note.tags.clone(),
            status: note.status,
            hash: note.hash.clone(),
            lang: note.lang.clone(),
            body: note.body.clone(),
            folded,
        }
    }
}

impl IndexEntry {
    pub fn to_note(&self) -> Note {
        Note {
            id: self.id.clone(),
            date: self.date.clone(),
            time: self.time.clone(),
            file: self.file.clone(),
            subject: self.subject.clone(),
            summary: self.summary.clone(),
            tags: self.tags.clone(),
            status: self.status,
            hash: self.hash.clone(),
            lang: self.lang.clone(),
            body: self.body.clone(),
        }
    }
}

/// Entries grouped by day, so one file's worth can be replaced wholesale when
/// that file changes.
#[derive(Debug, Default)]
pub struct Index {
    by_date: BTreeMap<String, Vec<IndexEntry>>,
}

impl Index {
    pub fn replace_day(&mut self, date: &str, entries: Vec<IndexEntry>) {
        if entries.is_empty() {
            self.by_date.remove(date);
        } else {
            self.by_date.insert(date.to_string(), entries);
        }
    }

    pub fn push(&mut self, entry: IndexEntry) {
        self.by_date
            .entry(entry.date.clone())
            .or_default()
            .push(entry);
    }

    /// Forget days that no longer have a file on disk.
    pub fn retain_days(&mut self, present: &HashSet<String>) -> bool {
        let before = self.by_date.len();
        self.by_date.retain(|date, _| present.contains(date));
        self.by_date.len() != before
    }

    /// Dates newest first, with how many notes each holds.
    pub fn days(&self) -> Vec<(String, usize)> {
        self.by_date
            .iter()
            .rev()
            .map(|(date, entries)| (date.clone(), entries.len()))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.by_date.values().map(Vec::len).sum()
    }

    /// How many notes carry each tag. Derived, like everything else here.
    pub fn tag_counts(&self) -> HashMap<String, u32> {
        let mut counts = HashMap::new();
        for tag in self.entries().flat_map(|entry| entry.tags.iter()) {
            *counts.entry(tag.clone()).or_insert(0) += 1;
        }
        counts
    }

    /// The tags of the notes labelled in `lang`, for the vocabulary the model
    /// reuses: offered tags in another language, it keeps writing that one.
    /// A note with no language recorded counts as English, the language
    /// notes were labelled in before it was.
    pub fn tag_counts_in(&self, lang: &str) -> HashMap<String, u32> {
        let mut counts = HashMap::new();
        let tags = self
            .entries()
            .filter(|entry| entry.lang.as_deref().unwrap_or(language::ENGLISH.code) == lang)
            .flat_map(|entry| entry.tags.iter());
        for tag in tags {
            *counts.entry(tag.clone()).or_insert(0) += 1;
        }
        counts
    }

    pub fn entries(&self) -> impl Iterator<Item = &IndexEntry> {
        self.by_date.values().flatten()
    }

    /// Fill in bodies for a day whose metadata came from the cache, which by
    /// design never holds them.
    fn attach_bodies(&mut self, date: &str, parsed: Vec<IndexEntry>) {
        let Some(cached) = self.by_date.get_mut(date) else {
            return;
        };
        for entry in cached {
            if let Some(fresh) = parsed.iter().find(|p| p.id == entry.id) {
                entry.body = fresh.body.clone();
                entry.folded = fresh.folded.clone();
            }
        }
    }

    /// Notes still waiting on enrichment, oldest day first, so the queue can
    /// be refilled at startup from what the markdown actually says.
    pub fn pending(&self) -> Vec<(String, String)> {
        self.by_date
            .values()
            .flatten()
            .filter(|entry| entry.status == Status::Pending)
            .map(|entry| (entry.id.clone(), entry.date.clone()))
            .collect()
    }

    pub fn to_jsonl(&self) -> String {
        let mut out = String::new();
        for entry in self.by_date.values().flatten() {
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
    root.join(".scratchnote").join("index.jsonl")
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
pub fn parse_day(path: &Path, date: &str) -> Vec<IndexEntry> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    daily_file::parse_notes(&contents, date, &relative_day_path(date))
        .iter()
        .map(IndexEntry::from)
        .collect()
}

/// Reparse every daily file under the root.
pub fn rebuild(root: &Path) -> Index {
    let mut index = Index::default();
    for (date, path) in daily_files(root) {
        index.replace_day(&date, parse_day(&path, &date));
    }
    index
}

/// Load the cache, reparsing any day whose file is newer than it and dropping
/// days whose file is gone. The flag says whether the result differs from what
/// is on disk, so the caller knows if it needs writing back.
pub fn load(root: &Path) -> (Index, bool) {
    let path = index_path(root);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return (rebuild(root), true);
    };

    let cached_at = modified_at(&path);
    let mut index = Index::from_jsonl(&raw);
    let files = daily_files(root);
    let mut changed = false;

    // Every file is read either way, because search needs the bodies in
    // memory (SPEC 6). The mtime only decides whose metadata to trust.
    for (date, file) in &files {
        if modified_at(file) > cached_at {
            index.replace_day(date, parse_day(file, date));
            changed = true;
        } else {
            index.attach_bodies(date, parse_day(file, date));
        }
    }

    let present: HashSet<String> = files.into_iter().map(|(date, _)| date).collect();
    changed |= index.retain_days(&present);

    (index, changed)
}

fn modified_at(path: &Path) -> SystemTime {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::daily_file::{append_note, body_hash};

    fn note(id: &str, date: &str, time: &str, body: &str) -> Note {
        Note {
            id: id.to_string(),
            date: date.to_string(),
            time: time.to_string(),
            file: relative_day_path(date),
            subject: None,
            summary: None,
            tags: Vec::new(),
            status: Status::Pending,
            hash: body_hash(body),
            lang: None,
            body: body.to_string(),
        }
    }

    fn scratch_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("scratchnote-index-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// Writes a day file the same way the writer would.
    fn write_day(root: &Path, date: &str, notes: &[Note]) {
        let path = super::super::day_path(root, date);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut contents = String::new();
        for note in notes {
            contents = append_note(&contents, note, date);
        }
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn jsonl_round_trips() {
        let mut index = Index::default();
        index.push(IndexEntry::from(&note("01AAA", "2026-09-22", "08:00", "a")));
        index.push(IndexEntry::from(&note("01BBB", "2026-09-23", "09:00", "b")));

        let back = Index::from_jsonl(&index.to_jsonl());
        assert_eq!(back.len(), 2);
        assert_eq!(
            back.days(),
            vec![("2026-09-23".into(), 1), ("2026-09-22".into(), 1)]
        );
    }

    #[test]
    fn a_line_matches_the_shape_in_the_spec() {
        let mut index = Index::default();
        index.push(IndexEntry::from(&note(
            "01AAA",
            "2026-09-22",
            "14:32",
            "body",
        )));
        let line = index.to_jsonl();
        let value: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

        for key in [
            "id", "date", "time", "file", "subject", "summary", "tags", "status", "hash",
        ] {
            assert!(value.get(key).is_some(), "missing {key} in {line}");
        }
        assert!(
            value.get("body").is_none(),
            "the index must not carry bodies"
        );
        assert_eq!(value["file"], "notes/2026/2026-09-22.md");
        assert_eq!(value["status"], "pending");
    }

    #[test]
    fn bodies_come_back_from_the_markdown_when_the_cache_is_fresh() {
        let root = scratch_root("fresh-cache-bodies");
        write_day(
            &root,
            "2026-09-22",
            &[note("01AAA", "2026-09-22", "08:00", "the body")],
        );
        let cache = index_path(&root);
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        std::fs::write(&cache, rebuild(&root).to_jsonl()).unwrap();

        let (index, changed) = load(&root);
        assert!(!changed, "a fresh cache needs no write back");
        let entry = index.entries().next().unwrap();
        assert_eq!(entry.body, "the body");
        assert!(entry.folded.contains("the body"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn counts_each_tag_once_per_note() {
        let mut index = Index::default();
        let mut a = note("01AAA", "2026-09-22", "08:00", "a");
        a.tags = vec!["infra".into(), "k8s".into()];
        let mut b = note("01BBB", "2026-09-23", "08:00", "b");
        b.tags = vec!["infra".into()];
        index.push(IndexEntry::from(&a));
        index.push(IndexEntry::from(&b));

        let counts = index.tag_counts();
        assert_eq!(counts["infra"], 2);
        assert_eq!(counts["k8s"], 1);
    }

    #[test]
    fn counts_a_languages_tags_with_unrecorded_notes_as_english() {
        let mut index = Index::default();
        let mut old = note("01AAA", "2026-09-22", "08:00", "a");
        old.tags = vec!["movie".into()];
        let mut french = note("01BBB", "2026-09-23", "08:00", "b");
        french.tags = vec!["film".into()];
        french.lang = Some("fr".into());
        index.push(IndexEntry::from(&old));
        index.push(IndexEntry::from(&french));

        assert_eq!(
            index.tag_counts_in("en").keys().collect::<Vec<_>>(),
            ["movie"]
        );
        assert_eq!(
            index.tag_counts_in("fr").keys().collect::<Vec<_>>(),
            ["film"]
        );
        assert!(index.tag_counts_in("de").is_empty());
        // The lang survives the index.jsonl cache.
        let back = Index::from_jsonl(&index.to_jsonl());
        assert_eq!(
            back.tag_counts_in("fr").keys().collect::<Vec<_>>(),
            ["film"]
        );
    }

    #[test]
    fn one_bad_line_costs_only_that_entry() {
        let mut index = Index::default();
        index.push(IndexEntry::from(&note("01AAA", "2026-09-22", "08:00", "a")));
        let raw = format!("not json at all\n{}", index.to_jsonl());
        assert_eq!(Index::from_jsonl(&raw).len(), 1);
    }

    #[test]
    fn days_are_newest_first_with_counts() {
        let mut index = Index::default();
        index.push(IndexEntry::from(&note("01AAA", "2026-09-22", "08:00", "a")));
        index.push(IndexEntry::from(&note("01BBB", "2026-09-22", "09:00", "b")));
        index.push(IndexEntry::from(&note("01CCC", "2026-09-24", "09:00", "c")));

        assert_eq!(
            index.days(),
            vec![("2026-09-24".into(), 1), ("2026-09-22".into(), 2)]
        );
    }

    #[test]
    fn rebuild_reads_every_daily_file() {
        let root = scratch_root("rebuild");
        write_day(
            &root,
            "2026-09-22",
            &[
                note("01AAA", "2026-09-22", "08:00", "first"),
                note("01BBB", "2026-09-22", "09:00", "second"),
            ],
        );
        write_day(
            &root,
            "2026-09-23",
            &[note("01CCC", "2026-09-23", "10:00", "third")],
        );

        let index = rebuild(&root);
        assert_eq!(index.len(), 3);
        assert_eq!(
            index.days(),
            vec![("2026-09-23".into(), 1), ("2026-09-22".into(), 2)]
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// SPEC 11: deleting `.scratchnote/` and relaunching restores the index.
    #[test]
    fn load_rebuilds_from_markdown_when_the_cache_is_missing() {
        let root = scratch_root("missing-cache");
        write_day(
            &root,
            "2026-09-22",
            &[note("01AAA", "2026-09-22", "08:00", "first")],
        );

        let (index, changed) = load(&root);
        assert!(changed, "a missing cache has to be written back");
        assert_eq!(index.len(), 1);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn load_reparses_a_day_whose_file_is_newer_than_the_cache() {
        let root = scratch_root("stale-cache");
        write_day(
            &root,
            "2026-09-22",
            &[note("01AAA", "2026-09-22", "08:00", "first")],
        );

        // A cache written before the file it describes.
        let cache = index_path(&root);
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        std::fs::write(&cache, "").unwrap();
        let old = SystemTime::now() - std::time::Duration::from_secs(60);
        filetime::set_file_mtime(&cache, filetime::FileTime::from_system_time(old)).unwrap();

        let (index, changed) = load(&root);
        assert!(changed);
        assert_eq!(
            index.len(),
            1,
            "the newer day file should have been reparsed"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn load_forgets_a_day_whose_file_is_gone() {
        let root = scratch_root("vanished-day");
        std::fs::create_dir_all(root.join("notes")).unwrap();

        let mut stale = Index::default();
        stale.push(IndexEntry::from(&note(
            "01AAA",
            "2026-09-22",
            "08:00",
            "gone",
        )));
        let cache = index_path(&root);
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        std::fs::write(&cache, stale.to_jsonl()).unwrap();

        let (index, changed) = load(&root);
        assert!(changed);
        assert_eq!(index.len(), 0);

        let _ = std::fs::remove_dir_all(&root);
    }
}
