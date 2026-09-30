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

use super::daily_file::{self, Kind, Note, Status};
use super::{check_date, page_file, relative_day_path};

/// One note or page. The body is held in memory for search (SPEC 6) but
/// never written to `index.jsonl`, which stays a small metadata cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    pub id: String,
    pub date: String,
    pub time: String,
    pub file: String,
    pub subject: Option<String>,
    pub category: Option<String>,
    pub status: Status,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// A page's `file` is its own and its `subject` its title (SPEC 4.7).
    #[serde(default, skip_serializing_if = "Kind::is_note")]
    pub kind: Kind,
    /// How many words the body holds, counted once here so the day list
    /// does not count every body again. `None` only on a line from a cache
    /// written before words were counted, whose day `load` reparses.
    #[serde(default)]
    pub words: Option<usize>,
    #[serde(skip)]
    pub body: String,
    /// Subject and body folded for matching, computed once here so a
    /// search does not fold ten thousand bodies per keystroke.
    #[serde(skip)]
    pub folded: String,
}

impl From<&Note> for IndexEntry {
    fn from(note: &Note) -> Self {
        let folded = crate::search::fold(&format!(
            "{}
{}",
            note.subject.as_deref().unwrap_or(""),
            note.body
        ));
        Self {
            id: note.id.clone(),
            date: note.date.clone(),
            time: note.time.clone(),
            file: note.file.clone(),
            subject: note.subject.clone(),
            category: note.category.clone(),
            status: note.status,
            hash: note.hash.clone(),
            lang: note.lang.clone(),
            kind: note.kind,
            words: Some(words(&note.body)),
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
            category: self.category.clone(),
            status: self.status,
            hash: self.hash.clone(),
            lang: self.lang.clone(),
            body: self.body.clone(),
            kind: self.kind,
            missing: false,
        }
    }

    /// Everything the cache holds, which leaves out the body.
    fn same_meta(&self, other: &IndexEntry) -> bool {
        IndexEntry {
            body: String::new(),
            folded: String::new(),
            ..self.clone()
        } == IndexEntry {
            body: String::new(),
            folded: String::new(),
            ..other.clone()
        }
    }
}

/// Notes grouped by day, so one file's worth can be replaced wholesale when
/// that file changes, and pages by id, since each has a file of its own.
#[derive(Debug, Default)]
pub struct Index {
    by_date: BTreeMap<String, Vec<IndexEntry>>,
    pages: BTreeMap<String, IndexEntry>,
}

impl Index {
    /// A day's notes. Its pages come from their own files, so they stay.
    pub fn replace_day(&mut self, date: &str, entries: Vec<IndexEntry>) {
        if entries.is_empty() {
            self.by_date.remove(date);
        } else {
            self.by_date.insert(date.to_string(), entries);
        }
    }

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

    /// Add a page, or replace it when its id is already there.
    pub fn replace_page(&mut self, entry: IndexEntry) {
        self.pages.insert(entry.id.clone(), entry);
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

    /// How many notes each category holds. Derived, like everything else here.
    pub fn category_counts(&self) -> HashMap<String, u32> {
        let mut counts = HashMap::new();
        for category in self.entries().filter_map(|entry| entry.category.as_ref()) {
            *counts.entry(category.clone()).or_insert(0) += 1;
        }
        counts
    }

    /// Notes day by day, then pages.
    pub fn entries(&self) -> impl Iterator<Item = &IndexEntry> {
        self.by_date.values().flatten().chain(self.pages.values())
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
        self.entries()
            .filter(|entry| entry.status == Status::Pending)
            .map(|entry| (entry.id.clone(), entry.date.clone()))
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
pub fn parse_page(root: &Path, path: &Path) -> Option<IndexEntry> {
    let contents = std::fs::read_to_string(path).ok()?;
    let note = page_file::parse_page(&contents, &relative(root, path)?)?;
    Some(IndexEntry::from(&note))
}

/// Every page under the root. A second file with an id already seen, such as
/// a copy made by hand, is left out.
fn parse_pages(root: &Path) -> Vec<IndexEntry> {
    let mut seen = HashSet::new();
    let mut pages = Vec::new();
    for path in page_files(root) {
        let Some(entry) = parse_page(root, &path) else {
            continue;
        };
        if seen.insert(entry.id.clone()) {
            pages.push(entry);
        } else {
            log::warn!("{} repeats page {}; skipping it", path.display(), entry.id);
        }
    }
    pages
}

/// The runs of text between spaces that hold a letter or a digit, so a
/// list's `-` or a task's box is not a word.
fn words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|token| !matches!(*token, "[x]" | "[X]"))
        .filter(|token| token.chars().any(char::is_alphanumeric))
        .count()
}

/// Reparse every daily file and page file under the root.
pub fn rebuild(root: &Path) -> Index {
    let mut index = Index::default();
    for (date, path) in daily_files(root) {
        index.replace_day(&date, parse_day(&path, &date));
    }
    for page in parse_pages(root) {
        index.replace_page(page);
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

    // A cache from before notes had a category instead of tags would read
    // as notes with none.
    if raw.contains("\"tags\":") {
        return (rebuild(root), true);
    }

    let cached_at = modified_at(&path);
    let mut index = Index::from_jsonl(&raw);
    let files = daily_files(root);
    let mut changed = false;

    // Every file is read either way, because search needs the bodies in
    // memory (SPEC 6). The mtime only decides whose metadata to trust, and a
    // day cached before words were counted is not trusted either.
    for (date, file) in &files {
        if modified_at(file) > cached_at || index.uncounted(date) {
            index.replace_day(date, parse_day(file, date));
            changed = true;
        } else {
            index.attach_bodies(date, parse_day(file, date));
        }
    }

    let present: HashSet<String> = files.into_iter().map(|(date, _)| date).collect();
    changed |= index.retain_days(&present);

    // Pages are few and each file is read anyway, so what they say replaces
    // the cache outright: a page renamed by hand keeps its mtime, which
    // would otherwise leave the cache pointing at its old name.
    let pages = parse_pages(root);
    let cached = std::mem::take(&mut index.pages);
    changed |= cached.len() != pages.len()
        || pages
            .iter()
            .any(|page| !cached.get(&page.id).is_some_and(|c| c.same_meta(page)));
    for page in pages {
        index.replace_page(page);
    }

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
            category: None,
            status: Status::Pending,
            hash: body_hash(body),
            lang: None,
            body: body.to_string(),
            kind: Kind::Note,
            missing: false,
        }
    }

    fn page(id: &str, date: &str, title: &str, body: &str) -> Note {
        Note {
            file: page_file::relative_path(date, &page_file::file_name(date, title, 1)),
            subject: Some(title.to_string()),
            kind: Kind::Page,
            ..note(id, date, "10:00", body)
        }
    }

    fn write_page(root: &Path, page: &Note) {
        let path = root.join(&page.file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, page_file::render_page(page)).unwrap();
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
            "id", "date", "time", "file", "subject", "category", "status", "hash",
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
    fn counts_the_notes_in_each_category() {
        let mut index = Index::default();
        let mut a = note("01AAA", "2026-09-22", "08:00", "a");
        a.category = Some("infra".into());
        let mut b = note("01BBB", "2026-09-23", "08:00", "b");
        b.category = Some("infra".into());
        let mut c = note("01CCC", "2026-09-23", "09:00", "c");
        c.category = Some("movie".into());
        for entry in [&a, &b, &c, &note("01DDD", "2026-09-23", "10:00", "d")] {
            index.push(IndexEntry::from(entry));
        }

        let counts = index.category_counts();
        assert_eq!(counts.len(), 2);
        assert_eq!(counts["infra"], 2);
        assert_eq!(counts["movie"], 1);
    }

    #[test]
    fn the_label_language_survives_the_cache() {
        let mut index = Index::default();
        let mut french = note("01BBB", "2026-09-23", "08:00", "b");
        french.lang = Some("fr".into());
        index.push(IndexEntry::from(&french));
        let back = Index::from_jsonl(&index.to_jsonl());
        assert_eq!(back.entries().next().unwrap().lang.as_deref(), Some("fr"));
    }

    #[test]
    fn a_cache_written_with_tags_is_rebuilt_from_the_markdown() {
        let root = scratch_root("tags-cache");
        let mut labelled = note("01AAA", "2026-09-22", "08:00", "the body");
        labelled.category = Some("movie".into());
        write_day(&root, "2026-09-22", &[labelled]);
        let cache = index_path(&root);
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        std::fs::write(
            &cache,
            r#"{"id":"01AAA","date":"2026-09-22","time":"08:00","file":"notes/2026/2026-09-22.md","subject":null,"summary":null,"tags":["movie"],"status":"pending","hash":"x"}"#,
        )
        .unwrap();

        let (index, changed) = load(&root);
        assert!(changed);
        assert_eq!(
            index.entries().next().unwrap().category.as_deref(),
            Some("movie")
        );

        let _ = std::fs::remove_dir_all(&root);
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
    fn words_leave_out_pages_and_markdown_marks() {
        let mut index = Index::default();
        index.push(IndexEntry::from(&note(
            "01AAA",
            "2026-09-22",
            "08:00",
            "# Groceries\n- [ ] buy milk\n- [x] call Anna, 2 times",
        )));
        index.push(IndexEntry::from(&note(
            "01BBB",
            "2026-09-22",
            "09:00",
            "> fine",
        )));
        index.push(IndexEntry::from(&page(
            "01CCC",
            "2026-09-22",
            "Plan",
            "long page text",
        )));

        assert_eq!(index.words_on("2026-09-22"), 8);
        assert_eq!(index.words_on("2026-09-23"), 0);
    }

    #[test]
    fn words_are_counted_once_and_kept_without_the_body() {
        let mut index = Index::default();
        let mut entry =
            IndexEntry::from(&note("01AAA", "2026-09-22", "08:00", "three small words"));
        entry.body.clear();
        entry.folded.clear();
        index.push(entry);
        assert_eq!(index.words_on("2026-09-22"), 3);

        let back = Index::from_jsonl(&index.to_jsonl());
        assert_eq!(back.words_on("2026-09-22"), 3);
    }

    #[test]
    fn a_cache_from_before_words_were_counted_has_its_days_counted() {
        let root = scratch_root("uncounted-cache");
        write_day(
            &root,
            "2026-09-22",
            &[note("01AAA", "2026-09-22", "08:00", "four words in here")],
        );
        // Written after the day file, so only the missing count makes it stale.
        let old: String = rebuild(&root)
            .to_jsonl()
            .lines()
            .map(|line| {
                let mut value: serde_json::Value = serde_json::from_str(line).unwrap();
                value.as_object_mut().unwrap().remove("words");
                format!("{value}\n")
            })
            .collect();
        let cache = index_path(&root);
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        std::fs::write(&cache, old).unwrap();

        let (index, changed) = load(&root);
        assert!(changed, "the counts have to be written back");
        assert_eq!(index.words_on("2026-09-22"), 4);

        let _ = std::fs::remove_dir_all(&root);
    }

    /// What a heavy writer's space costs to load, for the memory plan: five
    /// years at 40 notes a day. Not a gate, a measure:
    /// `cargo test --release heavy_writer -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn a_heavy_writers_space_loads() {
        const DAYS: usize = 5 * 365;
        const NOTES_A_DAY: usize = 40;
        let root = scratch_root("heavy-writer");
        let start = chrono::NaiveDate::from_ymd_opt(2021, 1, 1).unwrap();
        let sentence = "Talked with the team about the deployment pipeline and the \
                        staging cluster, then wrote down what to try next week. ";
        for day in 0..DAYS {
            let date = (start + chrono::Days::new(day as u64))
                .format("%Y-%m-%d")
                .to_string();
            let notes: Vec<Note> = (0..NOTES_A_DAY)
                .map(|n| {
                    let body = format!("Note {n} of {date}. {}", sentence.repeat(1 + n % 8));
                    let time = format!("{:02}:{:02}", 8 + n / 4, (n % 4) * 15);
                    note(&format!("{day:05}{n:02}"), &date, &time, &body)
                })
                .collect();
            write_day(&root, &date, &notes);
        }
        let cache = index_path(&root);
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        std::fs::write(&cache, rebuild(&root).to_jsonl()).unwrap();

        let started = std::time::Instant::now();
        let (index, changed) = load(&root);
        let took = started.elapsed();
        assert!(!changed);

        let held: usize = index
            .entries()
            .map(|entry| entry.body.len() + entry.folded.len())
            .sum();
        eprintln!(
            "{} notes, fresh cache loaded in {took:?}, {:.1} MB of note text held",
            index.len(),
            held as f64 / 1_000_000.0
        );

        let _ = std::fs::remove_dir_all(&root);
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
    fn pages_are_indexed_from_their_files_and_counted_on_their_day() {
        let root = scratch_root("pages");
        write_day(
            &root,
            "2026-09-22",
            &[note("01AAA", "2026-09-22", "08:00", "a note")],
        );
        write_page(
            &root,
            &page("01PPP", "2026-09-22", "Weekly sync", "the meeting"),
        );
        write_page(&root, &page("01QQQ", "2026-09-24", "Retro", "went well"));
        // Not a page: no marker.
        std::fs::write(root.join("pages/2026/loose.md"), "# loose\n").unwrap();

        let index = rebuild(&root);
        assert_eq!(index.len(), 3);
        assert_eq!(
            index.days(),
            vec![("2026-09-24".into(), 1), ("2026-09-22".into(), 2)]
        );
        let sync = index.page("01PPP").unwrap();
        assert_eq!(sync.file, "pages/2026/2026-09-22 Weekly sync.md");
        assert_eq!(sync.subject.as_deref(), Some("Weekly sync"));
        assert!(sync.folded.contains("weekly sync") && sync.folded.contains("the meeting"));
        assert_eq!(
            index.page_at(&sync.file).map(|p| p.id.as_str()),
            Some("01PPP")
        );
        assert_eq!(index.pages_on("2026-09-24").count(), 1);

        // A day's reparse is about its notes and leaves its pages alone.
        let mut index = index;
        index.replace_day("2026-09-22", Vec::new());
        assert!(index.page("01PPP").is_some());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_kind_survives_the_cache_and_notes_leave_it_out() {
        let mut index = Index::default();
        index.push(IndexEntry::from(&note("01AAA", "2026-09-22", "08:00", "a")));
        index.push(IndexEntry::from(&page("01PPP", "2026-09-22", "Sync", "b")));
        let jsonl = index.to_jsonl();
        let lines: Vec<&str> = jsonl.lines().collect();
        assert!(!lines[0].contains("kind"), "{}", lines[0]);
        assert!(lines[1].contains(r#""kind":"page""#), "{}", lines[1]);

        let back = Index::from_jsonl(&jsonl);
        assert_eq!(back.page("01PPP").map(|p| p.kind), Some(Kind::Page));
        assert_eq!(back.days(), vec![("2026-09-22".into(), 2)]);
    }

    #[test]
    fn load_follows_a_page_renamed_by_hand() {
        let root = scratch_root("renamed-page");
        let sync = page("01PPP", "2026-09-22", "Weekly sync", "the meeting");
        write_page(&root, &sync);
        let cache = index_path(&root);
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        std::fs::write(&cache, rebuild(&root).to_jsonl()).unwrap();

        let from = root.join(&sync.file);
        let to = root.join("pages/2026/sync.md");
        std::fs::rename(&from, &to).unwrap();
        // A rename keeps the mtime, so the cache still looks fresh.
        let old = SystemTime::now() - std::time::Duration::from_secs(60);
        filetime::set_file_mtime(&to, filetime::FileTime::from_system_time(old)).unwrap();

        let (index, changed) = load(&root);
        assert!(changed, "the cache has to be written back");
        let moved = index.page("01PPP").unwrap();
        assert_eq!(moved.file, "pages/2026/sync.md");
        assert_eq!(moved.body, "the meeting");

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
