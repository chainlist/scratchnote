//! Threads, SPEC 6.4: the notes about one thing, gathered across days.
//!
//! A note joins the thread it is closest to, scored against the average of
//! the thread's notes once what every note shares is taken off, as Similar
//! notes scores two notes (SPEC 6.2). A note close to no thread, but close
//! enough to a note on its own, starts a thread with it. Only notes within
//! a window of days of a thread, or of each other, are compared: a thread
//! follows one stretch of something rather than a subject for good, and a
//! pass stays short however many notes the space holds.
//!
//! Derived like `vectors.bin`: `threads.json` records where each note was
//! placed and from which body, so a note is placed once, and again only
//! when its text changes. A file that is missing, damaged or made from
//! another model's vectors places every note again, oldest first. What the
//! user decides, a thread's title or a note kept out of threads, is in
//! `thread-edits.json`, which nothing derives.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::{Days, NaiveDate};
use serde::{Deserialize, Serialize};

use super::vectors::{dot, Vectors};
use crate::storage::index::Index;

/// Where placing draws its lines.
#[derive(Debug, Clone, Copy)]
pub struct Cuts {
    /// The score a note needs against a thread to join it.
    pub join: f32,
    /// The score two notes on their own need to start a thread.
    pub pair: f32,
    /// How many days a note may lie beyond a thread's first or last note
    /// and still join it, and two notes on their own lie apart and still
    /// start one.
    pub window: u64,
}

/// Starting a thread takes more than joining one: two notes are a thinner
/// sign than a thread's worth.
pub const CUTS: Cuts = Cuts {
    join: 0.35,
    pair: 0.40,
    window: 60,
};

/// Bumped when the way notes are placed changes, so they are placed again.
const VERSION: u32 = 1;

/// No thread forms in a space of fewer notes: one thread's notes would be
/// most of what is usual, and with that taken off, every other note looks
/// like every other. They are placed once the space has grown.
pub const MIN_NOTES: usize = 12;

pub fn threads_path(root: &Path) -> PathBuf {
    root.join(".scratchnote").join("threads.json")
}

pub fn edits_path(root: &Path) -> PathBuf {
    root.join(".scratchnote").join("thread-edits.json")
}

/// When a note was written, which orders the notes and spaces them in days.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct When {
    pub date: NaiveDate,
    pub time: String,
}

/// When each note and page of the index was written.
pub fn when_written(index: &Index) -> HashMap<String, When> {
    index
        .entries()
        .filter_map(|entry| {
            let date = NaiveDate::parse_from_str(&entry.date, "%Y-%m-%d").ok()?;
            let when = When {
                date,
                time: entry.time.clone(),
            };
            Some((entry.id.clone(), when))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Placed {
    /// The body hash the note was placed from.
    hash: String,
    /// Its thread, left out for a note on its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thread: Option<String>,
    /// Placed while the user kept it out of threads, so it is placed again
    /// once they let it back.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    out: bool,
}

/// `threads.json`: where every note of the space was placed.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Threads {
    version: u32,
    /// The model the vectors placed from came from.
    model: String,
    notes: BTreeMap<String, Placed>,
}

/// What the user decided about threads, which placing never overrides.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Edits {
    /// Titles given to threads, by thread.
    pub titles: BTreeMap<String, String>,
    /// Notes taken out of threads, which stay out.
    pub alone: BTreeSet<String>,
}

impl Edits {
    /// A missing or unreadable file decides nothing.
    pub fn load(root: &Path) -> Self {
        std::fs::read_to_string(edits_path(root))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// A thread as the views show it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Thread {
    /// The id of the note it started with, which it keeps.
    pub id: String,
    /// The user's title, else the subject of the note most like the rest of
    /// the thread. `None` while no note of it has a subject.
    pub title: Option<String>,
    /// The title is the user's.
    pub named: bool,
    /// Its notes and pages, oldest first.
    pub notes: Vec<String>,
    /// The day of its first note.
    pub since: String,
}

/// A thread while notes are placed: what its notes add up to, how many
/// they are, and the days they span.
struct Group {
    sum: Vec<f32>,
    count: usize,
    /// sum · sum and sum · the space's sum, kept as the thread grows.
    own: f32,
    all: f32,
    first: NaiveDate,
    last: NaiveDate,
}

impl Group {
    fn new(dims: usize, date: NaiveDate) -> Self {
        Self {
            sum: vec![0.0; dims],
            count: 0,
            own: 0.0,
            all: 0.0,
            first: date,
            last: date,
        }
    }

    fn add(&mut self, vector: &[f32], date: NaiveDate, usual: &Usual) {
        for (s, x) in self.sum.iter_mut().zip(vector) {
            *s += x;
        }
        self.count += 1;
        self.own = dot(&self.sum, &self.sum);
        self.all = dot(&self.sum, &usual.sum);
        self.first = self.first.min(date);
        self.last = self.last.max(date);
    }

    fn part(&self) -> Part<'_> {
        Part {
            sum: &self.sum,
            count: self.count,
            own: self.own,
            all: self.all,
        }
    }

    /// Whether a note of `date` lies within `window` days of this thread's.
    fn near(&self, date: NaiveDate, window: u64) -> bool {
        earliest(self.first, window) <= date && date <= latest(self.last, window)
    }
}

/// The first day `window` days before `date`.
fn earliest(date: NaiveDate, window: u64) -> NaiveDate {
    date.checked_sub_days(Days::new(window))
        .unwrap_or(NaiveDate::MIN)
}

/// The last day `window` days after `date`.
fn latest(date: NaiveDate, window: u64) -> NaiveDate {
    date.checked_add_days(Days::new(window))
        .unwrap_or(NaiveDate::MAX)
}

/// A note or a thread as it is scored: what it adds up to, how many notes
/// that is, and the two dot products of its sum that placing a note leaves
/// alone, so that each comparison costs only one more.
struct Part<'a> {
    sum: &'a [f32],
    count: usize,
    /// sum · sum
    own: f32,
    /// sum · the space's sum
    all: f32,
}

/// What every note of the space adds up to.
fn total(vectors: &Vectors) -> Vec<f32> {
    let mut sum = vec![0.0; vectors.dims()];
    for (_, _, vector) in vectors.iter() {
        for (s, x) in sum.iter_mut().zip(vector) {
            *s += x;
        }
    }
    sum
}

/// The average note of the space, which says what is usual.
fn mean(vectors: &Vectors) -> Vec<f32> {
    let count = vectors.len().max(1) as f32;
    total(vectors).into_iter().map(|s| s / count).collect()
}

/// What every note of the space adds up to, which says what is usual, and
/// each note's dot products with itself and that sum.
struct Usual {
    sum: Vec<f32>,
    /// sum · sum
    ss: f32,
    count: usize,
    dots: HashMap<String, (f32, f32)>,
}

impl Usual {
    fn of(vectors: &Vectors) -> Self {
        let sum = total(vectors);
        let dots = vectors
            .iter()
            .map(|(id, _, vector)| (id.to_string(), (dot(vector, vector), dot(vector, &sum))))
            .collect();
        Self {
            ss: dot(&sum, &sum),
            sum,
            count: vectors.len(),
            dots,
        }
    }

    /// A note of the space as it is scored.
    fn note<'a>(&self, id: &str, vector: &'a [f32]) -> Part<'a> {
        let (own, all) = self
            .dots
            .get(id)
            .copied()
            .unwrap_or_else(|| (dot(vector, vector), dot(vector, &self.sum)));
        Part {
            sum: vector,
            count: 1,
            own,
            all,
        }
    }

    /// The cosine between note `a` and the average of the notes of `group`,
    /// once the mean of every other note is taken off both: what
    /// `Vectors::similar` gives for two notes, `group` being one. `a` and
    /// the group's notes are counted in the space's sum. Below any threshold
    /// when too few notes are left to say what is usual.
    fn score(&self, a: &Part, group: &Part) -> f32 {
        let rest = self.count as f32 - 1.0 - group.count as f32;
        if rest < 1.0 {
            return f32::NEG_INFINITY;
        }
        let k = group.count as f32;
        let (aa, a_s, a_g) = (a.own, a.all, dot(a.sum, group.sum));
        let (g_s, gg) = (group.all, group.own);

        // As in `Vectors::closest`, expanded rather than centred copies,
        // with m = (S - a - G) / rest and the group's average c = G / k.
        let ac = a_g / k;
        let cc = gg / (k * k);
        let am = (a_s - aa - a_g) / rest;
        let cm = (g_s - a_g - gg) / (k * rest);
        let mm = (self.ss + aa + gg - 2.0 * a_s - 2.0 * g_s + 2.0 * a_g) / (rest * rest);
        let norm = (aa - 2.0 * am + mm).max(0.0).sqrt() * (cc - 2.0 * cm + mm).max(0.0).sqrt();
        if norm > 0.0 {
            (ac - am - cm + mm) / norm
        } else {
            f32::NEG_INFINITY
        }
    }
}

/// Where a note goes.
enum Choice {
    Join(String),
    Pair(String),
}

impl Threads {
    /// The saved placements, or none, which places every note again. Never
    /// an error: the file can always be rebuilt.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    /// Temp file, fsync, rename, as `Vectors::save`, which it sits beside:
    /// the watcher never looks at it.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(self).map_err(io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)
    }

    /// The thread a note is in, if any.
    #[cfg(test)]
    pub fn of(&self, id: &str) -> Option<&str> {
        self.notes.get(id)?.thread.as_deref()
    }

    /// Place every note of `vectors` not placed yet, oldest first, and place
    /// again those written again since, and those the user took out of
    /// threads, in `alone`, or let back in. Notes gone are forgotten. A note
    /// `when` does not know is left for a later pass. True when anything
    /// changed.
    pub fn reconcile(
        &mut self,
        vectors: &Vectors,
        when: &HashMap<String, When>,
        alone: &HashSet<String>,
    ) -> bool {
        self.reconcile_with(vectors, when, alone, CUTS)
    }

    /// `reconcile`, with the lines drawn at `cuts`.
    pub fn reconcile_with(
        &mut self,
        vectors: &Vectors,
        when: &HashMap<String, When>,
        alone: &HashSet<String>,
        cuts: Cuts,
    ) -> bool {
        let mut changed = false;
        if self.version != VERSION || self.model != vectors.model_id() {
            *self = Threads {
                version: VERSION,
                model: vectors.model_id().to_string(),
                notes: BTreeMap::new(),
            };
            changed = true;
        }

        let before = self.notes.len();
        self.notes.retain(|id, placed| {
            vectors.get(id).is_some_and(|(hash, _)| hash == placed.hash)
                && placed.out == alone.contains(id)
        });
        // A thread down to one note is no thread: that note is placed again.
        let mut sizes: HashMap<String, usize> = HashMap::new();
        for thread in self
            .notes
            .values()
            .filter_map(|placed| placed.thread.clone())
        {
            *sizes.entry(thread).or_default() += 1;
        }
        self.notes.retain(|_, placed| {
            placed
                .thread
                .as_ref()
                .is_none_or(|thread| sizes[thread] > 1)
        });
        changed |= self.notes.len() != before;

        let mut todo: Vec<(&When, &str)> = vectors
            .iter()
            .filter(|(id, _, _)| !self.notes.contains_key(*id))
            .filter_map(|(id, _, _)| Some((when.get(id)?, id)))
            .collect();
        if todo.is_empty() || vectors.len() < MIN_NOTES {
            return changed;
        }
        todo.sort();

        let usual = Usual::of(vectors);
        // Ordered, so equal scores always pick the same thread.
        let mut groups: BTreeMap<String, Group> = BTreeMap::new();
        // The notes on their own that a note may start a thread with, by day.
        let mut single: BTreeSet<(NaiveDate, String)> = BTreeSet::new();
        for (id, placed) in &self.notes {
            let (Some(written), Some((_, vector))) = (when.get(id), vectors.get(id)) else {
                continue;
            };
            match &placed.thread {
                Some(thread) => groups
                    .entry(thread.clone())
                    .or_insert_with(|| Group::new(vectors.dims(), written.date))
                    .add(vector, written.date, &usual),
                None if !alone.contains(id) => {
                    single.insert((written.date, id.clone()));
                }
                None => {}
            }
        }

        // Threads that formed or grew, which may take in a note on its own.
        let mut touched: BTreeSet<String> = BTreeSet::new();
        for (written, id) in todo {
            let (hash, vector) = vectors.get(id).expect("listed from the vectors");
            let choice = if alone.contains(id) {
                None
            } else {
                let note = usual.note(id, vector);
                best(&note, written.date, &usual, &groups, &single, vectors, cuts)
            };
            let thread = match choice {
                Some(Choice::Join(thread)) => {
                    if let Some(group) = groups.get_mut(&thread) {
                        group.add(vector, written.date, &usual);
                    }
                    touched.insert(thread.clone());
                    Some(thread)
                }
                Some(Choice::Pair(other)) => {
                    let (Some(their), Some((_, theirs))) = (when.get(&other), vectors.get(&other))
                    else {
                        continue;
                    };
                    // Named after the earlier of the two, unless a thread
                    // already goes by that name.
                    let mut names = [other.as_str(), id];
                    if (written, id) < (their, other.as_str()) {
                        names.reverse();
                    }
                    let thread = names
                        .iter()
                        .map(|name| name.to_string())
                        .chain((2..).map(|n| format!("{}-{n}", names[0])))
                        .find(|name| !groups.contains_key(name))
                        .expect("a free name");
                    let mut group = Group::new(vectors.dims(), their.date);
                    group.add(theirs, their.date, &usual);
                    group.add(vector, written.date, &usual);
                    groups.insert(thread.clone(), group);
                    single.remove(&(their.date, other.clone()));
                    if let Some(placed) = self.notes.get_mut(&other) {
                        placed.thread = Some(thread.clone());
                    }
                    touched.insert(thread.clone());
                    Some(thread)
                }
                None => {
                    if !alone.contains(id) {
                        single.insert((written.date, id.to_string()));
                    }
                    None
                }
            };
            let placed = Placed {
                hash: hash.to_string(),
                thread,
                out: alone.contains(id),
            };
            self.notes.insert(id.to_string(), placed);
        }

        // A note on its own was placed before the thread it fits formed, or
        // before the thread grew towards it: it joins now, which may draw
        // the thread nearer others in turn.
        while !touched.is_empty() {
            let near: BTreeSet<(NaiveDate, String)> = touched
                .iter()
                .filter_map(|thread| groups.get(thread))
                .flat_map(|group| {
                    let last = latest(group.last, cuts.window);
                    single
                        .range((earliest(group.first, cuts.window), String::new())..)
                        .take_while(move |(day, _)| *day <= last)
                        .cloned()
                })
                .collect();
            touched.clear();
            for (day, id) in near {
                let Some((_, vector)) = vectors.get(&id) else {
                    continue;
                };
                let note = usual.note(&id, vector);
                let Some((_, thread)) = best_thread(&note, day, &usual, &groups, cuts) else {
                    continue;
                };
                if let Some(group) = groups.get_mut(&thread) {
                    group.add(vector, day, &usual);
                }
                single.remove(&(day, id.clone()));
                if let Some(placed) = self.notes.get_mut(&id) {
                    placed.thread = Some(thread.clone());
                }
                touched.insert(thread);
            }
        }
        true
    }

    /// Every thread, its notes oldest first. `subjects` holds the notes'
    /// subjects, for the thread's title when the user gave it none.
    pub fn list(
        &self,
        vectors: &Vectors,
        when: &HashMap<String, When>,
        subjects: &HashMap<String, String>,
        titles: &BTreeMap<String, String>,
    ) -> Vec<Thread> {
        let mut members: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (id, placed) in &self.notes {
            if let (Some(thread), true) = (&placed.thread, when.contains_key(id)) {
                members
                    .entry(thread.as_str())
                    .or_default()
                    .push(id.as_str());
            }
        }
        let mean = mean(vectors);
        members
            .into_iter()
            .filter_map(|(id, notes)| shown(id, notes, vectors, &mean, when, subjects, titles))
            .collect()
    }

    /// One thread as `list` gives it, without titling every other, or
    /// `None` once it is gone.
    pub fn get(
        &self,
        id: &str,
        vectors: &Vectors,
        when: &HashMap<String, When>,
        subjects: &HashMap<String, String>,
        titles: &BTreeMap<String, String>,
    ) -> Option<Thread> {
        let notes: Vec<&str> = self
            .notes
            .iter()
            .filter(|(note, placed)| {
                placed.thread.as_deref() == Some(id) && when.contains_key(*note)
            })
            .map(|(note, _)| note.as_str())
            .collect();
        shown(id, notes, vectors, &mean(vectors), when, subjects, titles)
    }
}

/// Thread `id` of `notes` as the views show it, or `None` for a thread of
/// one note, which is no thread.
fn shown(
    id: &str,
    mut notes: Vec<&str>,
    vectors: &Vectors,
    mean: &[f32],
    when: &HashMap<String, When>,
    subjects: &HashMap<String, String>,
    titles: &BTreeMap<String, String>,
) -> Option<Thread> {
    if notes.len() < 2 {
        return None;
    }
    notes.sort_by(|a, b| (&when[*a], a).cmp(&(&when[*b], b)));
    let custom = titles.get(id).filter(|title| !title.trim().is_empty());
    let title = custom
        .cloned()
        .or_else(|| most_typical(&notes, vectors, mean, subjects));
    Some(Thread {
        id: id.to_string(),
        title,
        named: custom.is_some(),
        since: when[notes[0]].date.format("%Y-%m-%d").to_string(),
        notes: notes.into_iter().map(str::to_string).collect(),
    })
}

/// Where a note of `date` goes: the thread it scores best against, or the
/// note on its own it scores best against, among those near enough in time
/// that clear their threshold. `None` keeps it on its own.
fn best(
    note: &Part,
    date: NaiveDate,
    usual: &Usual,
    groups: &BTreeMap<String, Group>,
    single: &BTreeSet<(NaiveDate, String)>,
    vectors: &Vectors,
    cuts: Cuts,
) -> Option<Choice> {
    let mut best: Option<(f32, Choice)> = best_thread(note, date, usual, groups, cuts)
        .map(|(score, thread)| (score, Choice::Join(thread)));
    let last = latest(date, cuts.window);
    for (day, other) in single.range((earliest(date, cuts.window), String::new())..) {
        if *day > last {
            break;
        }
        let Some((_, theirs)) = vectors.get(other) else {
            continue;
        };
        let score = usual.score(note, &usual.note(other, theirs));
        if score >= cuts.pair && best.as_ref().is_none_or(|(top, _)| score > *top) {
            best = Some((score, Choice::Pair(other.clone())));
        }
    }
    best.map(|(_, choice)| choice)
}

/// The thread near `date` a note scores best against, if it clears the cut
/// to join, with its score.
fn best_thread(
    note: &Part,
    date: NaiveDate,
    usual: &Usual,
    groups: &BTreeMap<String, Group>,
    cuts: Cuts,
) -> Option<(f32, String)> {
    let mut best: Option<(f32, String)> = None;
    let near = groups
        .iter()
        .filter(|(_, group)| group.near(date, cuts.window));
    for (thread, group) in near {
        let score = usual.score(note, &group.part());
        if score >= cuts.join && best.as_ref().is_none_or(|(top, _)| score > *top) {
            best = Some((score, thread.clone()));
        }
    }
    best
}

/// The subject of the note most like the rest of its thread, once what
/// every note shares is taken off: the one that best says what the thread
/// is about. A note without a subject yet gives way to the next. `mean` is
/// the space's average note.
fn most_typical(
    notes: &[&str],
    vectors: &Vectors,
    mean: &[f32],
    subjects: &HashMap<String, String>,
) -> Option<String> {
    let dims = vectors.dims();
    let centred =
        |vector: &[f32]| -> Vec<f32> { vector.iter().zip(mean).map(|(x, m)| x - m).collect() };
    let mut middle = vec![0.0; dims];
    let found: Vec<(&str, Vec<f32>)> = notes
        .iter()
        .filter_map(|id| Some((*id, centred(vectors.get(id)?.1))))
        .collect();
    for (_, vector) in &found {
        for (s, x) in middle.iter_mut().zip(vector) {
            *s += x;
        }
    }
    let mut ranked: Vec<(f32, &str)> = found
        .iter()
        .map(|(id, vector)| {
            let norm = dot(vector, vector).sqrt();
            let score = if norm > 0.0 {
                dot(vector, &middle) / norm
            } else {
                f32::NEG_INFINITY
            };
            (score, *id)
        })
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    ranked
        .into_iter()
        .find_map(|(_, id)| subjects.get(id).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every note leans on the first axis, as real notes share a lot; the
    /// axis a note names after that is what it is about.
    fn vector(about: usize, dims: usize) -> Vec<f32> {
        let mut v = vec![0.0; dims];
        v[0] = 1.0;
        v[about] = 1.0;
        v
    }

    /// Notes as `(id, date, the axis it is about)`, spread over enough other
    /// axes that a mean says something.
    fn space(notes: &[(&str, &str, usize)]) -> (Vectors, HashMap<String, When>) {
        let mut vectors = Vectors::new("m", 16);
        let mut when = HashMap::new();
        for (id, date, about) in notes {
            vectors
                .insert(id.to_string(), format!("h{id}"), vector(*about, 16))
                .unwrap();
            let written = When {
                date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
                time: "09:00".to_string(),
            };
            when.insert(id.to_string(), written);
        }
        (vectors, when)
    }

    /// Unrelated notes, one per axis, to say what is usual.
    const OTHERS: [(&str, &str, usize); 10] = [
        ("01Z1", "2026-09-01", 6),
        ("01Z2", "2026-09-02", 7),
        ("01Z3", "2026-09-03", 8),
        ("01Z4", "2026-09-04", 9),
        ("01Z5", "2026-09-05", 10),
        ("01Z6", "2026-09-06", 11),
        ("01Z7", "2026-09-07", 12),
        ("01Z8", "2026-09-08", 13),
        ("01Z9", "2026-09-09", 14),
        ("01ZA", "2026-09-10", 15),
    ];

    fn with_others(
        notes: &[(&'static str, &'static str, usize)],
    ) -> Vec<(&'static str, &'static str, usize)> {
        notes.iter().chain(OTHERS.iter()).copied().collect()
    }

    fn threads_of(
        threads: &Threads,
        vectors: &Vectors,
        when: &HashMap<String, When>,
    ) -> Vec<Vec<String>> {
        threads
            .list(vectors, when, &HashMap::new(), &BTreeMap::new())
            .into_iter()
            .map(|thread| thread.notes)
            .collect()
    }

    #[test]
    fn notes_about_one_thing_make_a_thread_and_the_rest_stay_on_their_own() {
        let notes = with_others(&[
            ("01A", "2026-09-10", 1),
            ("01B", "2026-09-12", 1),
            ("01C", "2026-09-20", 1),
            ("01D", "2026-09-15", 2),
        ]);
        let (vectors, when) = space(&notes);
        let mut threads = Threads::default();
        assert!(threads.reconcile(&vectors, &when, &HashSet::new()));
        assert_eq!(
            threads_of(&threads, &vectors, &when),
            vec![vec!["01A", "01B", "01C"]]
        );
        // Named after the note it started with.
        assert_eq!(threads.of("01C"), Some("01A"));
        assert_eq!(threads.of("01D"), None);

        // Nothing new, nothing changes.
        assert!(!threads.reconcile(&vectors, &when, &HashSet::new()));
    }

    #[test]
    fn a_note_long_after_a_thread_starts_its_own() {
        let notes = with_others(&[
            ("01A", "2026-01-10", 1),
            ("01B", "2026-01-12", 1),
            // Well past the window after the thread's last note.
            ("01C", "2026-06-01", 1),
            ("01D", "2026-06-03", 1),
        ]);
        let (vectors, when) = space(&notes);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &HashSet::new());
        assert_eq!(
            threads_of(&threads, &vectors, &when),
            vec![vec!["01A", "01B"], vec!["01C", "01D"]]
        );
    }

    #[test]
    fn a_note_is_placed_again_once_edited_and_a_thread_of_one_is_none() {
        let notes = with_others(&[
            ("01A", "2026-09-10", 1),
            ("01B", "2026-09-12", 1),
            ("01C", "2026-09-14", 1),
        ]);
        let (mut vectors, when) = space(&notes);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &HashSet::new());

        // 01C now says something else.
        vectors
            .insert("01C".into(), "edited".into(), vector(3, 16))
            .unwrap();
        assert!(threads.reconcile(&vectors, &when, &HashSet::new()));
        assert_eq!(
            threads_of(&threads, &vectors, &when),
            vec![vec!["01A", "01B"]]
        );
        assert_eq!(threads.of("01C"), None);

        // 01B deleted: 01A is left on its own.
        let present: HashSet<String> = notes
            .iter()
            .map(|(id, ..)| id.to_string())
            .filter(|id| id != "01B")
            .collect();
        vectors.retain(&present);
        assert!(threads.reconcile(&vectors, &when, &HashSet::new()));
        assert!(threads_of(&threads, &vectors, &when).is_empty());
        assert_eq!(threads.of("01A"), None);
    }

    #[test]
    fn a_note_taken_out_stays_out_and_starts_nothing() {
        let notes = with_others(&[
            ("01A", "2026-09-10", 1),
            ("01B", "2026-09-12", 1),
            ("01C", "2026-09-14", 1),
        ]);
        let (vectors, when) = space(&notes);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &HashSet::new());

        let alone: HashSet<String> = ["01B".to_string()].into();
        assert!(threads.reconcile(&vectors, &when, &alone));
        assert_eq!(
            threads_of(&threads, &vectors, &when),
            vec![vec!["01A", "01C"]]
        );

        // Taken out with the thread down to two, the other is left alone too.
        let alone: HashSet<String> = ["01B".to_string(), "01C".to_string()].into();
        threads.reconcile(&vectors, &when, &alone);
        assert!(threads_of(&threads, &vectors, &when).is_empty());

        // Let back in, they gather again.
        assert!(threads.reconcile(&vectors, &when, &HashSet::new()));
        assert_eq!(
            threads_of(&threads, &vectors, &when),
            vec![vec!["01A", "01B", "01C"]]
        );
    }

    #[test]
    fn another_model_places_every_note_again_and_the_file_round_trips() {
        let root = std::env::temp_dir().join("scratchnote-threads-round-trip");
        let _ = std::fs::remove_dir_all(&root);
        let path = threads_path(&root);
        let notes = with_others(&[("01A", "2026-09-10", 1), ("01B", "2026-09-12", 1)]);
        let (vectors, when) = space(&notes);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &HashSet::new());
        threads.save(&path).unwrap();
        assert_eq!(Threads::load(&path), threads);
        assert!(!path.with_extension("json.tmp").exists());

        let mut other = Vectors::new("other", 16);
        for (id, _, about) in &notes {
            other
                .insert(id.to_string(), format!("h{id}"), vector(*about, 16))
                .unwrap();
        }
        let mut loaded = Threads::load(&path);
        assert!(loaded.reconcile(&other, &when, &HashSet::new()));
        assert_eq!(loaded.model, "other");
        assert_eq!(threads_of(&loaded, &other, &when), vec![vec!["01A", "01B"]]);

        // A broken file is no threads at all.
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(Threads::load(&path), Threads::default());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_thread_is_titled_by_the_user_or_after_its_most_typical_note() {
        let notes = with_others(&[
            ("01A", "2026-09-10", 1),
            ("01B", "2026-09-12", 1),
            ("01C", "2026-09-14", 1),
        ]);
        let (mut vectors, when) = space(&notes);
        // 01B leans a little off the thread, so 01A or 01C says it best.
        let mut off = vector(1, 16);
        off[4] = 0.6;
        vectors.insert("01B".into(), "h01B".into(), off).unwrap();
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &HashSet::new());

        let subjects: HashMap<String, String> = [
            ("01A", "Kitchen quotes"),
            ("01B", "Plumber and tiles"),
            ("01C", "Kitchen finished"),
        ]
        .into_iter()
        .map(|(id, s)| (id.to_string(), s.to_string()))
        .collect();
        let listed = threads.list(&vectors, &when, &subjects, &BTreeMap::new());
        assert_eq!(listed.len(), 1);
        assert!(!listed[0].named);
        assert_ne!(listed[0].title.as_deref(), Some("Plumber and tiles"));

        // Without subjects yet, no title; the user's wins over everything.
        assert_eq!(
            threads.list(&vectors, &when, &HashMap::new(), &BTreeMap::new())[0].title,
            None
        );
        let titles: BTreeMap<String, String> = [("01A".to_string(), "Kitchen".to_string())].into();
        let named = threads.list(&vectors, &when, &subjects, &titles);
        assert_eq!(named[0].title.as_deref(), Some("Kitchen"));
        assert!(named[0].named);
    }

    #[test]
    fn one_thread_is_got_as_it_is_listed() {
        let notes = with_others(&[
            ("01A", "2026-09-10", 1),
            ("01B", "2026-09-12", 1),
            ("01C", "2026-09-14", 2),
            ("01D", "2026-09-16", 2),
        ]);
        let (vectors, when) = space(&notes);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &HashSet::new());
        let subjects: HashMap<String, String> = [("01A", "Kitchen"), ("01D", "Garden")]
            .into_iter()
            .map(|(id, s)| (id.to_string(), s.to_string()))
            .collect();
        let titles: BTreeMap<String, String> = [("01C".to_string(), "Mine".to_string())].into();

        let listed = threads.list(&vectors, &when, &subjects, &titles);
        assert_eq!(listed.len(), 2);
        for thread in &listed {
            let got = threads.get(&thread.id, &vectors, &when, &subjects, &titles);
            assert_eq!(got.as_ref(), Some(thread));
        }
        // A note on its own is no thread.
        assert_eq!(
            threads.get("01Z1", &vectors, &when, &subjects, &titles),
            None
        );
    }

    #[test]
    fn a_space_too_small_to_say_what_is_usual_waits() {
        let few = [
            ("01A", "2026-09-10", 1),
            ("01B", "2026-09-12", 1),
            ("01Z1", "2026-09-01", 9),
            ("01Z2", "2026-09-02", 10),
        ];
        let (vectors, when) = space(&few);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &HashSet::new());
        assert!(threads_of(&threads, &vectors, &when).is_empty());
        assert!(threads.notes.is_empty(), "nothing placed yet");

        let (vectors, when) = space(&with_others(&[
            ("01A", "2026-09-10", 1),
            ("01B", "2026-09-12", 1),
        ]));
        assert!(threads.reconcile(&vectors, &when, &HashSet::new()));
        assert_eq!(
            threads_of(&threads, &vectors, &when),
            vec![vec!["01A", "01B"]]
        );
    }

    #[test]
    fn a_note_without_a_day_waits_for_a_later_pass() {
        let notes = with_others(&[("01A", "2026-09-10", 1), ("01B", "2026-09-12", 1)]);
        let (vectors, mut when) = space(&notes);
        when.remove("01B");
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &HashSet::new());
        assert!(threads_of(&threads, &vectors, &when).is_empty());

        let (_, when) = space(&notes);
        assert!(threads.reconcile(&vectors, &when, &HashSet::new()));
        assert_eq!(
            threads_of(&threads, &vectors, &when),
            vec![vec!["01A", "01B"]]
        );
    }

    #[test]
    fn edits_load_empty_without_a_file() {
        let root = std::env::temp_dir().join("scratchnote-thread-edits-missing");
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(Edits::load(&root), Edits::default());
    }

    /// The made-up notes, placed with the real model: every thread is about
    /// one thing, and nearly every note about a thing is in its thread, while
    /// a note on its own stays so. Needs the embedding model;
    /// `cargo test threads_gather -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the downloaded embedding model"]
    fn threads_gather_the_notes_about_each_thing() {
        use crate::embed::{installed_embedder, samples, Embedder};

        let Some(embedder) = installed_embedder() else {
            return;
        };
        let notes = samples::notes();
        let mut vectors = Vectors::new(embedder.model_id(), embedder.dims());
        let mut when = HashMap::new();
        for (_, note) in &notes {
            let vector = embedder.embed_document(&note.body).unwrap();
            vectors
                .insert(note.id.clone(), note.hash.clone(), vector)
                .unwrap();
            let written = When {
                date: NaiveDate::parse_from_str(&note.date, "%Y-%m-%d").unwrap(),
                time: note.time.clone(),
            };
            when.insert(note.id.clone(), written);
        }
        let about: HashMap<&str, &str> = notes
            .iter()
            .map(|(thing, note)| (note.id.as_str(), *thing))
            .collect();

        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &HashSet::new());
        let listed = threads.list(&vectors, &when, &HashMap::new(), &BTreeMap::new());

        let mut threaded = 0;
        for thread in &listed {
            let things: BTreeSet<&str> = thread.notes.iter().map(|id| about[id.as_str()]).collect();
            eprintln!("{things:?} {}", thread.notes.len());
            assert_eq!(things.len(), 1, "a thread mixes {things:?}");
            assert!(
                !things.iter().any(|thing| thing.starts_with('-')),
                "{things:?}"
            );
            threaded += thread.notes.len();
        }
        let on_a_thing = notes
            .iter()
            .filter(|(thing, _)| !thing.starts_with('-'))
            .count();
        assert!(
            threaded * 10 >= on_a_thing * 9,
            "{threaded} of {on_a_thing} threaded"
        );
    }
}

#[cfg(test)]
mod speed {
    use super::*;

    /// A first pass places every note of a space, which for a heavy writer
    /// is many thousands. Only a gate in release, as the other timing tests,
    /// and a tenth of the notes in a debug build, which is many times slower:
    /// `cargo test --release placing_is_fast -- --nocapture`.
    #[test]
    fn placing_is_fast_enough_for_ten_thousand_notes() {
        const DIMS: usize = 768;
        let count: u64 = if cfg!(debug_assertions) {
            1_000
        } else {
            10_000
        };
        let mut state: u32 = 0x9e37_79b9;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as f32 / u32::MAX as f32 - 0.5
        };
        let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let mut vectors = Vectors::new("m", DIMS);
        let mut when = HashMap::new();
        // Fourteen notes a day for two years, about nothing in common.
        for i in 0..count {
            let id = format!("{i:05}");
            let vector: Vec<f32> = (0..DIMS).map(|_| next()).collect();
            vectors.insert(id.clone(), "h".into(), vector).unwrap();
            let date = start.checked_add_days(Days::new(i / 14)).unwrap();
            when.insert(
                id,
                When {
                    date,
                    time: format!("{:02}:00", i % 14 + 8),
                },
            );
        }

        let started = std::time::Instant::now();
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &HashSet::new());
        let took = started.elapsed();
        eprintln!("placed {count} notes in {took:?}");
        if !cfg!(debug_assertions) {
            assert!(took.as_secs_f32() < 5.0, "took {took:?}");
        }
    }
}
