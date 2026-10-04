//! Threads, SPEC 6.4: the notes about one thing, gathered across days.
//!
//! A note joins the thread it is closest to, scored against the average of
//! the thread's notes once what every note shares is taken off, as Similar
//! notes scores two notes (SPEC 6.2). A note close to no thread, but close
//! enough to a note on its own, starts a thread with it. Two threads that
//! grow close enough become one. Only notes within a window of days of a
//! thread, or of each other, are compared: a thread follows one stretch of
//! something rather than a subject for good, and a pass stays short however
//! many notes the space holds.
//!
//! Taking off the mean misleads in a small space, or one mostly about one
//! thing: the mean is then mostly that thing, and every other note shares
//! not being about it. So a note and a thread must also stand out for each
//! other, each against its own average cosine with every other note, which
//! no shared mean sways.
//!
//! Derived like the vectors: the `placed` table of `space.db` records where
//! each note was placed and from which body, so a note is placed once, and
//! again only when its text changes. Placements that are missing or made
//! from another model's vectors place every note again, oldest first. What
//! the user decides, a thread's title or a note kept out of threads, is in
//! the thread edit tables beside it, which nothing derives.
//!
//! `threads.json` and `thread-edits.json`, the files they were saved in
//! before, are read once when `space.db` is made.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use chrono::{Days, NaiveDate};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use super::vectors::{dot, Vectors};
use crate::storage::index::Index;
use crate::storage::space_db::{to_string, SpaceDb};

/// Where placing draws its lines.
#[derive(Debug, Clone, Copy)]
pub struct Cuts {
    /// The score a note needs against a thread to join it.
    pub join: f32,
    /// The score two notes on their own need to start a thread.
    pub pair: f32,
    /// How far above its usual cosine with the other notes each side must
    /// score the other, whether to join, start or merge.
    pub lead: f32,
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
    lead: 0.14,
    window: 60,
};

/// Bumped when the way notes are placed changes, so they are placed again.
const VERSION: u32 = 2;

/// Two notes leave no other to say what is usual, so nothing is placed
/// until a third comes.
const MIN_NOTES: usize = 3;

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

/// Where every note of the space was placed. The fields are those of
/// `threads.json`, which is read in once.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Threads {
    version: u32,
    /// The model the vectors placed from came from.
    model: String,
    notes: BTreeMap<String, Placed>,
    /// The placements as last loaded or saved, so a save writes only the
    /// notes placed since. `None` until they are, which writes them all.
    #[serde(skip)]
    saved: Option<Saved>,
}

#[derive(Debug, Clone, Default)]
struct Saved {
    version: u32,
    model: String,
    notes: BTreeMap<String, Placed>,
}

impl Saved {
    fn is_of(&self, version: u32, model: &str) -> bool {
        self.version == version && self.model == model
    }
}

/// The same placements, however much of them is saved.
impl PartialEq for Threads {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version && self.model == other.model && self.notes == other.notes
    }
}

/// What the user decided about threads, which placing never overrides.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Edits {
    /// Titles given to threads, by thread.
    pub titles: BTreeMap<String, String>,
    /// Notes taken out of threads, which stay out.
    pub alone: BTreeSet<String>,
    /// Notes the user put in a thread, by note, which stay there whatever
    /// they score. A thread holding one is the user's, as is a titled one;
    /// any other is only suggested.
    pub pinned: BTreeMap<String, String>,
    /// Suggested threads the user dismissed, with the notes each held then.
    /// One is suggested again once it holds another.
    pub dismissed: BTreeMap<String, BTreeSet<String>>,
}

impl Edits {
    /// The threads that are the user's: titled, or holding a note they put
    /// there.
    pub fn kept(&self) -> HashSet<&str> {
        self.titles
            .iter()
            .filter(|(_, title)| !title.trim().is_empty())
            .map(|(thread, _)| thread.as_str())
            .chain(self.pinned.values().map(String::as_str))
            .collect()
    }

    /// What the user decided. Edits that cannot be read decide nothing.
    pub fn load(db: &SpaceDb) -> Self {
        Self::read(db.conn()).unwrap_or_else(|e| {
            log::warn!("could not read the thread edits: {e}");
            Self::default()
        })
    }

    fn read(conn: &Connection) -> rusqlite::Result<Self> {
        let mut edits = Self::default();
        let pairs = |sql: &str| -> rusqlite::Result<Vec<(String, String)>> {
            let mut statement = conn.prepare(sql)?;
            let rows = statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect();
            rows
        };
        edits.titles = pairs("SELECT thread, title FROM thread_titles")?
            .into_iter()
            .collect();
        edits.pinned = pairs("SELECT note, thread FROM pinned")?
            .into_iter()
            .collect();
        for (thread, note) in pairs("SELECT thread, note FROM dismissed")? {
            edits.dismissed.entry(thread).or_default().insert(note);
        }
        let mut statement = conn.prepare("SELECT note FROM kept_alone")?;
        edits.alone = statement
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(edits)
    }

    /// Save these in place of the edits saved before.
    pub fn save(&self, db: &mut SpaceDb) -> Result<(), String> {
        db.transaction(|tx| self.write(tx))
    }

    /// What `save` writes, in the caller's transaction. A suggestion
    /// dismissed with no notes is not kept: holding any note, it is
    /// suggested again anyway.
    pub(crate) fn write(&self, tx: &Connection) -> Result<(), String> {
        tx.execute_batch(
            "DELETE FROM thread_titles; DELETE FROM kept_alone;
             DELETE FROM pinned; DELETE FROM dismissed;",
        )
        .map_err(to_string)?;
        let run = |sql: &str, a: &str, b: Option<&str>| -> Result<(), String> {
            let mut statement = tx.prepare_cached(sql).map_err(to_string)?;
            match b {
                Some(b) => statement.execute([a, b]),
                None => statement.execute([a]),
            }
            .map(|_| ())
            .map_err(to_string)
        };
        for (thread, title) in &self.titles {
            run(
                "INSERT INTO thread_titles (thread, title) VALUES (?1, ?2)",
                thread,
                Some(title),
            )?;
        }
        for note in &self.alone {
            run("INSERT INTO kept_alone (note) VALUES (?1)", note, None)?;
        }
        for (note, thread) in &self.pinned {
            run(
                "INSERT INTO pinned (note, thread) VALUES (?1, ?2)",
                note,
                Some(thread),
            )?;
        }
        for (thread, notes) in &self.dismissed {
            for note in notes {
                run(
                    "INSERT INTO dismissed (thread, note) VALUES (?1, ?2)",
                    thread,
                    Some(note),
                )?;
            }
        }
        Ok(())
    }

    /// The edits `thread-edits.json` holds, `None` for a file that is
    /// missing or unreadable.
    pub fn read_json(path: &Path) -> Option<Self> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }
}

/// A thread as the views show it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Thread {
    /// The id of the note it started with, which it keeps.
    pub id: String,
    /// The user's title. `None` until they give it one.
    pub title: Option<String>,
    /// The title is the user's.
    pub named: bool,
    /// The thread is the user's rather than only suggested.
    pub kept: bool,
    /// Its notes and pages, oldest first.
    pub notes: Vec<String>,
    /// The day of its first note.
    pub since: String,
    /// The day of its last note.
    pub until: String,
}

/// A thread while notes are placed: its notes, what they add up to, and the
/// days they span.
struct Group {
    notes: Vec<String>,
    sum: Vec<f32>,
    /// sum · sum and sum · the space's sum, kept as the thread grows.
    own: f32,
    all: f32,
    first: NaiveDate,
    last: NaiveDate,
}

impl Group {
    fn new(dims: usize, date: NaiveDate) -> Self {
        Self {
            notes: Vec::new(),
            sum: vec![0.0; dims],
            own: 0.0,
            all: 0.0,
            first: date,
            last: date,
        }
    }

    fn add(&mut self, id: &str, vector: &[f32], date: NaiveDate, usual: &Usual) {
        for (s, x) in self.sum.iter_mut().zip(vector) {
            *s += x;
        }
        self.notes.push(id.to_string());
        self.own = dot(&self.sum, &self.sum);
        self.all = dot(&self.sum, &usual.sum);
        self.first = self.first.min(date);
        self.last = self.last.max(date);
    }

    /// Take in the notes of `other`.
    fn absorb(&mut self, other: Group, usual: &Usual) {
        for (s, x) in self.sum.iter_mut().zip(&other.sum) {
            *s += x;
        }
        self.notes.extend(other.notes);
        self.own = dot(&self.sum, &self.sum);
        self.all = dot(&self.sum, &usual.sum);
        self.first = self.first.min(other.first);
        self.last = self.last.max(other.last);
    }

    fn part(&self) -> Part<'_> {
        Part {
            sum: &self.sum,
            count: self.notes.len(),
            own: self.own,
            all: self.all,
        }
    }

    /// Whether a note of `date` lies within `window` days of this thread's.
    fn near(&self, date: NaiveDate, window: u64) -> bool {
        earliest(self.first, window) <= date && date <= latest(self.last, window)
    }

    /// Whether some note of `other` lies within `window` days of this
    /// thread's.
    fn meets(&self, other: &Group, window: u64) -> bool {
        earliest(self.first, window) <= other.last && other.first <= latest(self.last, window)
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
        let sum = vectors.sum();
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

    /// The cosine between the averages of `a` and `b`, a note or a thread
    /// each, once the mean of every other note is taken off both: what
    /// `Vectors::similar` gives for two notes. `None` unless each also
    /// scores the other `lead` above its average cosine with every other
    /// note, or when too few notes are left to say what is usual. The notes
    /// of `a` and `b` are counted in the space's sum.
    fn score(&self, a: &Part, b: &Part, lead: f32) -> Option<f32> {
        let rest = self.count as f32 - (a.count + b.count) as f32;
        if rest < 1.0 {
            return None;
        }
        let (ka, kb) = (a.count as f32, b.count as f32);
        let (aa, bb, ab) = (a.own, b.own, dot(a.sum, b.sum));

        // Each side's average cosine with the other notes, S - A - B, and
        // how far above it each scores the other. A mean that is mostly one
        // thing sways neither.
        let (na, nb) = (aa.sqrt(), bb.sqrt());
        let cos = ab / (na * nb);
        let usual_a = (a.all - aa - ab) / (na * rest);
        let usual_b = (b.all - ab - bb) / (nb * rest);
        if cos - usual_a.max(usual_b) < lead {
            return None;
        }

        // As in `Vectors::closest`, expanded rather than centred copies,
        // with m = (S - A - B) / rest and the averages A / ka and B / kb.
        let am = (a.all - aa - ab) / rest;
        let bm = (b.all - ab - bb) / rest;
        let mm = (self.ss + aa + bb - 2.0 * a.all - 2.0 * b.all + 2.0 * ab) / (rest * rest);
        let xx = aa / (ka * ka) - 2.0 * am / ka + mm;
        let yy = bb / (kb * kb) - 2.0 * bm / kb + mm;
        let norm = xx.max(0.0).sqrt() * yy.max(0.0).sqrt();
        (norm > 0.0).then(|| (ab / (ka * kb) - am / ka - bm / kb + mm) / norm)
    }
}

/// Where a note goes.
enum Choice {
    Join(String),
    Pair(String),
    /// Where the user put it, which need not exist yet.
    Put(String),
}

impl Threads {
    /// The saved placements, or none, which places every note again. Never
    /// an error: they can always be placed again.
    pub fn load(db: &SpaceDb) -> Self {
        let version = db.meta("threads_version").and_then(|v| v.parse().ok());
        let (Some(version), Some(model)) = (version, db.meta("threads_model")) else {
            return Self::default();
        };
        let notes = match Self::read(db.conn()) {
            Ok(notes) => notes,
            Err(e) => {
                log::warn!("could not read where notes were placed: {e}");
                return Self::default();
            }
        };
        Self {
            saved: Some(Saved {
                version,
                model: model.clone(),
                notes: notes.clone(),
            }),
            version,
            model,
            notes,
        }
    }

    fn read(conn: &Connection) -> rusqlite::Result<BTreeMap<String, Placed>> {
        let mut statement = conn.prepare("SELECT id, hash, thread, out FROM placed")?;
        let rows = statement
            .query_map([], |row| {
                let placed = Placed {
                    hash: row.get(1)?,
                    thread: row.get(2)?,
                    out: row.get(3)?,
                };
                Ok((row.get(0)?, placed))
            })?
            .collect();
        rows
    }

    /// Write the notes placed since the last save, all of it in one go.
    pub fn save(&mut self, db: &mut SpaceDb) -> Result<(), String> {
        let changes = self.changes();
        db.transaction(|tx| self.write_changes(tx, changes.as_deref()))?;
        match (changes, self.saved.as_mut()) {
            (Some(changes), Some(saved)) => {
                for (id, placed) in changes {
                    match placed {
                        Some(placed) => saved.notes.insert(id, placed),
                        None => saved.notes.remove(&id),
                    };
                }
            }
            _ => {
                self.saved = Some(Saved {
                    version: self.version,
                    model: self.model.clone(),
                    notes: self.notes.clone(),
                });
            }
        }
        Ok(())
    }

    /// The notes placed or forgotten since the last save, each with where
    /// it is now. `None` when every placement is to be written: none is
    /// saved yet, or they were made another way. Both maps are in id order,
    /// so one pass along them finds every difference.
    fn changes(&self) -> Option<Vec<(String, Option<Placed>)>> {
        let saved = self
            .saved
            .as_ref()
            .filter(|saved| saved.is_of(self.version, &self.model))?;
        let mut changes = Vec::new();
        let mut now = self.notes.iter().peekable();
        let mut then = saved.notes.iter().peekable();
        loop {
            let order = match (now.peek(), then.peek()) {
                (None, None) => break,
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some((a, _)), Some((b, _))) => a.cmp(b),
            };
            match order {
                std::cmp::Ordering::Less => {
                    let (id, placed) = now.next().expect("peeked");
                    changes.push((id.clone(), Some(placed.clone())));
                }
                std::cmp::Ordering::Greater => {
                    let (id, _) = then.next().expect("peeked");
                    changes.push((id.clone(), None));
                }
                std::cmp::Ordering::Equal => {
                    let ((id, placed), (_, before)) =
                        (now.next().expect("peeked"), then.next().expect("peeked"));
                    if placed != before {
                        changes.push((id.clone(), Some(placed.clone())));
                    }
                }
            }
        }
        Some(changes)
    }

    /// What `save` writes, in the caller's transaction.
    pub(crate) fn write(&self, tx: &Connection) -> Result<(), String> {
        self.write_changes(tx, self.changes().as_deref())
    }

    /// Write `changes`, or every placement when there are none to go by.
    fn write_changes(
        &self,
        tx: &Connection,
        changes: Option<&[(String, Option<Placed>)]>,
    ) -> Result<(), String> {
        let mut put = tx
            .prepare_cached(
                "INSERT OR REPLACE INTO placed (id, hash, thread, out) VALUES (?1, ?2, ?3, ?4)",
            )
            .map_err(to_string)?;
        let mut row = |id: &str, placed: &Placed| -> Result<(), String> {
            put.execute(rusqlite::params![
                id,
                placed.hash,
                placed.thread,
                placed.out
            ])
            .map(|_| ())
            .map_err(to_string)
        };
        match changes {
            Some(changes) => changes.iter().try_for_each(|(id, placed)| match placed {
                Some(placed) => row(id, placed),
                None => tx
                    .execute("DELETE FROM placed WHERE id = ?1", [id])
                    .map(|_| ())
                    .map_err(to_string),
            }),
            None => {
                tx.execute("DELETE FROM placed", []).map_err(to_string)?;
                SpaceDb::set_meta(tx, "threads_version", &self.version.to_string())?;
                SpaceDb::set_meta(tx, "threads_model", &self.model)?;
                self.notes
                    .iter()
                    .try_for_each(|(id, placed)| row(id, placed))
            }
        }
    }

    /// The placements `threads.json` holds, `None` for a file that is
    /// missing or unreadable. The file was saved through serde.
    pub fn read_json(path: &Path) -> Option<Self> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }

    /// The file `read_json` reads, as the app saved it before `space.db`.
    #[cfg(test)]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    /// The thread a note is in, if any.
    pub fn of(&self, id: &str) -> Option<&str> {
        self.notes.get(id)?.thread.as_deref()
    }

    /// Place every note of `vectors` not placed yet, oldest first, and place
    /// again those written again since, and those the user took out of
    /// threads, let back in or put in a thread. A note the user put in a
    /// thread goes there first, whatever it scores, so the notes like it
    /// join. Notes gone are forgotten. A note `when` does not know is left
    /// for a later pass. Two threads of the user's never become one. True
    /// when anything changed.
    pub fn reconcile(
        &mut self,
        vectors: &Vectors,
        when: &HashMap<String, When>,
        edits: &Edits,
    ) -> bool {
        self.reconcile_with(vectors, when, edits, CUTS)
    }

    /// `reconcile`, with the lines drawn at `cuts`.
    pub fn reconcile_with(
        &mut self,
        vectors: &Vectors,
        when: &HashMap<String, When>,
        edits: &Edits,
        cuts: Cuts,
    ) -> bool {
        let alone = &edits.alone;
        let mut changed = false;
        if self.version != VERSION || self.model != vectors.model_id() {
            self.version = VERSION;
            self.model = vectors.model_id().to_string();
            self.notes.clear();
            changed = true;
        }

        let before = self.notes.len();
        self.notes.retain(|id, placed| {
            vectors.get(id).is_some_and(|(hash, _)| hash == placed.hash)
                && placed.out == alone.contains(id)
                && edits
                    .pinned
                    .get(id)
                    .is_none_or(|thread| placed.thread.as_ref() == Some(thread))
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
        // Notes put in a thread by the user first, so the others can join.
        todo.sort_by_key(|(written, id)| (!edits.pinned.contains_key(*id), *written, *id));

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
                    .add(id, vector, written.date, &usual),
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
            } else if let Some(thread) = edits.pinned.get(id) {
                Some(Choice::Put(thread.clone()))
            } else {
                let note = usual.note(id, vector);
                best(&note, written.date, &usual, &groups, &single, vectors, cuts)
            };
            let thread = match choice {
                Some(Choice::Put(thread)) => {
                    groups
                        .entry(thread.clone())
                        .or_insert_with(|| Group::new(vectors.dims(), written.date))
                        .add(id, vector, written.date, &usual);
                    touched.insert(thread.clone());
                    Some(thread)
                }
                Some(Choice::Join(thread)) => {
                    if let Some(group) = groups.get_mut(&thread) {
                        group.add(id, vector, written.date, &usual);
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
                    group.add(&other, theirs, their.date, &usual);
                    group.add(id, vector, written.date, &usual);
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

        let kept = edits.kept();

        // A note on its own was placed before the thread it fits formed, or
        // before the thread grew towards it: it joins now. A thread that
        // grew towards another becomes one with it. Either may draw a
        // thread nearer others in turn.
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
            let grown = std::mem::take(&mut touched);
            for (day, id) in near {
                let Some((_, vector)) = vectors.get(&id) else {
                    continue;
                };
                let note = usual.note(&id, vector);
                let Some((_, thread)) = best_thread(&note, day, &usual, &groups, cuts) else {
                    continue;
                };
                if let Some(group) = groups.get_mut(&thread) {
                    group.add(&id, vector, day, &usual);
                }
                single.remove(&(day, id.clone()));
                if let Some(placed) = self.notes.get_mut(&id) {
                    placed.thread = Some(thread.clone());
                }
                touched.insert(thread);
            }
            for thread in grown {
                let Some(other) = closest_thread(&thread, &usual, &groups, vectors, &kept, cuts)
                else {
                    continue;
                };
                // The user's goes on, else the older one.
                let older = |a: &str, b: &str| (groups[a].first, a) < (groups[b].first, b);
                let (keep, gone) = if kept.contains(other.as_str())
                    || (!kept.contains(thread.as_str()) && older(&other, &thread))
                {
                    (other, thread)
                } else {
                    (thread, other)
                };
                let absorbed = groups.remove(&gone).expect("a thread just scored");
                if let Some(group) = groups.get_mut(&keep) {
                    group.absorb(absorbed, &usual);
                }
                for placed in self.notes.values_mut() {
                    if placed.thread.as_deref() == Some(gone.as_str()) {
                        placed.thread = Some(keep.clone());
                    }
                }
                touched.remove(&gone);
                touched.insert(keep);
            }
        }
        true
    }

    /// Every thread, its notes oldest first, with the titles the user gave,
    /// but for suggestions the user dismissed that hold no other note since.
    pub fn list(&self, when: &HashMap<String, When>, edits: &Edits) -> Vec<Thread> {
        let mut members: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (id, placed) in &self.notes {
            if let (Some(thread), true) = (&placed.thread, when.contains_key(id)) {
                members
                    .entry(thread.as_str())
                    .or_default()
                    .push(id.as_str());
            }
        }
        let kept = edits.kept();
        members
            .into_iter()
            .filter_map(|(id, notes)| shown(id, notes, when, edits, &kept))
            .filter(|thread| {
                thread.kept
                    || edits
                        .dismissed
                        .get(&thread.id)
                        .is_none_or(|then| thread.notes.iter().any(|note| !then.contains(note)))
            })
            .collect()
    }

    /// The ids of every thread, those of one note included.
    pub fn names(&self) -> HashSet<String> {
        self.notes
            .values()
            .filter_map(|placed| placed.thread.clone())
            .collect()
    }

    /// The threads a note could be put in, the one it fits best first,
    /// whatever their days and however low it scores: the user chooses.
    /// Not the one it is in.
    pub fn ranked_for(&self, id: &str, vectors: &Vectors) -> Vec<String> {
        let Some((_, vector)) = vectors.get(id) else {
            return Vec::new();
        };
        let usual = Usual::of(vectors);
        let mut groups: BTreeMap<&str, Group> = BTreeMap::new();
        let mine = self.of(id);
        for (note, placed) in &self.notes {
            let (Some(thread), Some((_, theirs))) = (placed.thread.as_deref(), vectors.get(note))
            else {
                continue;
            };
            if note != id && Some(thread) != mine {
                groups
                    .entry(thread)
                    .or_insert_with(|| Group::new(vectors.dims(), NaiveDate::MIN))
                    .add(note, theirs, NaiveDate::MIN, &usual);
            }
        }
        let note = usual.note(id, vector);
        let mut ranked: Vec<(f32, &str)> = groups
            .iter()
            .filter(|(_, group)| group.notes.len() > 1)
            .map(|(thread, group)| {
                let score = usual
                    .score(&note, &group.part(), f32::NEG_INFINITY)
                    .unwrap_or(f32::NEG_INFINITY);
                (score, *thread)
            })
            .collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(b.1)));
        ranked
            .into_iter()
            .map(|(_, thread)| thread.to_string())
            .collect()
    }

    /// One thread as `list` gives it, without gathering every other, or
    /// `None` once it is gone. A dismissed suggestion is still got.
    pub fn get(&self, id: &str, when: &HashMap<String, When>, edits: &Edits) -> Option<Thread> {
        let notes: Vec<&str> = self
            .notes
            .iter()
            .filter(|(note, placed)| {
                placed.thread.as_deref() == Some(id) && when.contains_key(*note)
            })
            .map(|(note, _)| note.as_str())
            .collect();
        shown(id, notes, when, edits, &edits.kept())
    }
}

/// Thread `id` of `notes` as the views show it, or `None` for a thread of
/// one note, which is no thread.
fn shown(
    id: &str,
    mut notes: Vec<&str>,
    when: &HashMap<String, When>,
    edits: &Edits,
    kept: &HashSet<&str>,
) -> Option<Thread> {
    if notes.len() < 2 {
        return None;
    }
    notes.sort_by(|a, b| (&when[*a], a).cmp(&(&when[*b], b)));
    let title = edits
        .titles
        .get(id)
        .filter(|title| !title.trim().is_empty())
        .cloned();
    let day = |note: &str| when[note].date.format("%Y-%m-%d").to_string();
    Some(Thread {
        id: id.to_string(),
        named: title.is_some(),
        title,
        kept: kept.contains(id),
        since: day(notes[0]),
        until: day(notes[notes.len() - 1]),
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
        let Some(score) = usual.score(note, &usual.note(other, theirs), cuts.lead) else {
            continue;
        };
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
        let Some(score) = usual.score(note, &group.part(), cuts.lead) else {
            continue;
        };
        if score >= cuts.join && best.as_ref().is_none_or(|(top, _)| score > *top) {
            best = Some((score, thread.clone()));
        }
    }
    best
}

/// The thread near `thread` in time that it would become one with, unless
/// both are the user's: of the two, every note of the smaller
/// clears the cut to join the larger, as it would have had it come after.
/// Two threads' averages are not compared: averaging smooths away what
/// sets each apart, and threads would chain into one.
fn closest_thread(
    thread: &str,
    usual: &Usual,
    groups: &BTreeMap<String, Group>,
    vectors: &Vectors,
    kept: &HashSet<&str>,
    cuts: Cuts,
) -> Option<String> {
    let group = groups.get(thread)?;
    let mut best: Option<(f32, &String)> = None;
    let near = groups
        .iter()
        .filter(|(other, theirs)| *other != thread && group.meets(theirs, cuts.window))
        .filter(|(other, _)| !(kept.contains(thread) && kept.contains(other.as_str())));
    for (other, theirs) in near {
        let (smaller, larger) = if group.notes.len() <= theirs.notes.len() {
            (group, theirs)
        } else {
            (theirs, group)
        };
        let mut lowest = f32::INFINITY;
        for id in &smaller.notes {
            let score = vectors
                .get(id)
                .and_then(|(_, vector)| {
                    usual.score(&usual.note(id, vector), &larger.part(), cuts.lead)
                })
                .unwrap_or(f32::NEG_INFINITY);
            lowest = lowest.min(score);
            if lowest < cuts.join {
                break;
            }
        }
        if lowest >= cuts.join && best.is_none_or(|(top, _)| lowest > top) {
            best = Some((lowest, other));
        }
    }
    best.map(|(_, other)| other.clone())
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

    fn titled(thread: &str, title: &str) -> Edits {
        Edits {
            titles: [(thread.to_string(), title.to_string())].into(),
            ..Edits::default()
        }
    }

    fn threads_of(threads: &Threads, when: &HashMap<String, When>) -> Vec<Vec<String>> {
        threads
            .list(when, &Edits::default())
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
        assert!(threads.reconcile(&vectors, &when, &Edits::default()));
        assert_eq!(threads_of(&threads, &when), vec![vec!["01A", "01B", "01C"]]);
        // Named after the note it started with.
        assert_eq!(threads.of("01C"), Some("01A"));
        assert_eq!(threads.of("01D"), None);

        // Nothing new, nothing changes.
        assert!(!threads.reconcile(&vectors, &when, &Edits::default()));
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
        threads.reconcile(&vectors, &when, &Edits::default());
        assert_eq!(
            threads_of(&threads, &when),
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
        threads.reconcile(&vectors, &when, &Edits::default());

        // 01C now says something else.
        vectors
            .insert("01C".into(), "edited".into(), vector(3, 16))
            .unwrap();
        assert!(threads.reconcile(&vectors, &when, &Edits::default()));
        assert_eq!(threads_of(&threads, &when), vec![vec!["01A", "01B"]]);
        assert_eq!(threads.of("01C"), None);

        // 01B deleted: 01A is left on its own.
        let present: HashSet<String> = notes
            .iter()
            .map(|(id, ..)| id.to_string())
            .filter(|id| id != "01B")
            .collect();
        vectors.retain(&present);
        assert!(threads.reconcile(&vectors, &when, &Edits::default()));
        assert!(threads_of(&threads, &when).is_empty());
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
        threads.reconcile(&vectors, &when, &Edits::default());

        let alone = Edits {
            alone: ["01B".to_string()].into(),
            ..Edits::default()
        };
        assert!(threads.reconcile(&vectors, &when, &alone));
        assert_eq!(threads_of(&threads, &when), vec![vec!["01A", "01C"]]);

        // Taken out with the thread down to two, the other is left alone too.
        let alone = Edits {
            alone: ["01B".to_string(), "01C".to_string()].into(),
            ..Edits::default()
        };
        threads.reconcile(&vectors, &when, &alone);
        assert!(threads_of(&threads, &when).is_empty());

        // Let back in, they gather again.
        assert!(threads.reconcile(&vectors, &when, &Edits::default()));
        assert_eq!(threads_of(&threads, &when), vec![vec!["01A", "01B", "01C"]]);
    }

    #[test]
    fn another_model_places_every_note_again_and_the_placements_round_trip() {
        let mut db = SpaceDb::in_memory().unwrap();
        let notes = with_others(&[("01A", "2026-09-10", 1), ("01B", "2026-09-12", 1)]);
        let (vectors, when) = space(&notes);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &Edits::default());
        threads.save(&mut db).unwrap();
        assert_eq!(Threads::load(&db), threads);

        let mut other = Vectors::new("other", 16);
        for (id, _, about) in &notes {
            other
                .insert(id.to_string(), format!("h{id}"), vector(*about, 16))
                .unwrap();
        }
        let mut loaded = Threads::load(&db);
        assert!(loaded.reconcile(&other, &when, &Edits::default()));
        assert_eq!(loaded.model, "other");
        assert_eq!(threads_of(&loaded, &when), vec![vec!["01A", "01B"]]);
        loaded.save(&mut db).unwrap();
        assert_eq!(Threads::load(&db), loaded);

        // A broken file is no threads at all.
        let path = std::env::temp_dir().join("scratchnote-threads-broken.json");
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(Threads::read_json(&path), None);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_save_writes_only_the_notes_placed_since() {
        let mut db = SpaceDb::in_memory().unwrap();
        let notes = with_others(&[
            ("01A", "2026-09-10", 1),
            ("01B", "2026-09-12", 1),
            ("01C", "2026-09-14", 1),
        ]);
        let (mut vectors, mut when) = space(&notes);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &Edits::default());
        threads.save(&mut db).unwrap();
        let rows = |db: &SpaceDb| db.conn().total_changes();
        let before = rows(&db);

        // One note joins the thread, one goes.
        vectors
            .insert("01Z".into(), "h01Z".into(), vector(1, 16))
            .unwrap();
        let written = When {
            date: NaiveDate::from_ymd_opt(2026, 9, 15).unwrap(),
            time: "09:00".into(),
        };
        when.insert("01Z".into(), written);
        let present = vectors
            .iter()
            .map(|(id, _, _)| id.to_string())
            .filter(|id| id != "01C")
            .collect();
        vectors.retain(&present);
        when.remove("01C");
        assert!(threads.reconcile(&vectors, &when, &Edits::default()));
        threads.save(&mut db).unwrap();
        assert_eq!(rows(&db), before + 2);
        assert_eq!(Threads::load(&db), threads);
        assert_eq!(threads_of(&threads, &when), vec![vec!["01A", "01B", "01Z"]]);
    }

    #[test]
    fn a_thread_has_no_title_until_the_user_gives_it_one() {
        let notes = with_others(&[
            ("01A", "2026-09-10", 1),
            ("01B", "2026-09-12", 1),
            ("01C", "2026-09-14", 1),
        ]);
        let (vectors, when) = space(&notes);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &Edits::default());

        let listed = threads.list(&when, &Edits::default());
        assert_eq!(listed.len(), 1);
        assert_eq!((listed[0].title.as_deref(), listed[0].named), (None, false));
        assert!(!listed[0].kept, "only suggested");

        assert_eq!(threads.list(&when, &titled("01A", "  "))[0].title, None);
        let named = threads.list(&when, &titled("01A", "Kitchen"));
        assert_eq!(named[0].title.as_deref(), Some("Kitchen"));
        assert!(named[0].named);
        assert!(named[0].kept, "a titled thread is the user's");
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
        threads.reconcile(&vectors, &when, &Edits::default());
        let edits = titled("01C", "Mine");

        let listed = threads.list(&when, &edits);
        assert_eq!(listed.len(), 2);
        for thread in &listed {
            let got = threads.get(&thread.id, &when, &edits);
            assert_eq!(got.as_ref(), Some(thread));
        }
        // A note on its own is no thread.
        assert_eq!(threads.get("01Z1", &when, &edits), None);
    }

    #[test]
    fn a_note_put_in_a_thread_stays_there_and_makes_it_the_users() {
        let notes = with_others(&[
            ("01A", "2026-09-10", 1),
            ("01B", "2026-09-12", 1),
            ("01C", "2026-09-14", 2),
            ("01D", "2026-09-15", 3),
            ("01E", "2026-09-16", 4),
        ]);
        let (vectors, when) = space(&notes);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &Edits::default());
        assert_eq!(threads.of("01C"), None);

        // Into a thread, and two notes into a new one named after the first.
        let edits = Edits {
            pinned: [
                ("01C".to_string(), "01A".to_string()),
                ("01D".to_string(), "01D".to_string()),
                ("01E".to_string(), "01D".to_string()),
            ]
            .into(),
            ..Edits::default()
        };
        assert!(threads.reconcile(&vectors, &when, &edits));
        let listed = threads.list(&when, &edits);
        let notes: Vec<&Vec<String>> = listed.iter().map(|thread| &thread.notes).collect();
        assert_eq!(notes, [&vec!["01A", "01B", "01C"], &vec!["01D", "01E"]]);
        assert!(listed.iter().all(|thread| thread.kept));

        // Edited, it stays where it was put.
        let mut vectors = vectors;
        vectors
            .insert("01C".into(), "edited".into(), vector(5, 16))
            .unwrap();
        threads.reconcile(&vectors, &when, &edits);
        assert_eq!(threads.of("01C"), Some("01A"));
    }

    #[test]
    fn a_dismissed_suggestion_comes_back_once_it_grows() {
        let edits = Edits {
            dismissed: [(
                "01A".to_string(),
                ["01A".to_string(), "01B".to_string()].into(),
            )]
            .into(),
            ..Edits::default()
        };
        let two = with_others(&[("01A", "2026-09-10", 1), ("01B", "2026-09-12", 1)]);
        let (vectors, when) = space(&two);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &edits);
        assert!(threads.list(&when, &edits).is_empty());
        assert!(threads.get("01A", &when, &edits).is_some(), "still got");

        let mut three = two.clone();
        three.push(("01C", "2026-09-14", 1));
        let (vectors, when) = space(&three);
        threads.reconcile(&vectors, &when, &edits);
        assert_eq!(threads.list(&when, &edits).len(), 1);
    }

    #[test]
    fn the_threads_a_note_could_go_in_are_ranked_by_fit() {
        let notes = with_others(&[
            ("01A", "2026-01-10", 1),
            ("01B", "2026-01-12", 1),
            ("01C", "2026-09-14", 2),
            ("01D", "2026-09-15", 2),
            // Far from both in time, closer to the first in meaning.
            ("01E", "2027-06-01", 1),
        ]);
        let (vectors, when) = space(&notes);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &Edits::default());
        assert_eq!(threads.ranked_for("01E", &vectors), ["01A", "01C"]);
        assert_eq!(threads.ranked_for("01A", &vectors), ["01C"], "not its own");
    }

    #[test]
    fn two_notes_wait_for_a_third_to_say_what_is_usual() {
        let two = [("01A", "2026-09-10", 1), ("01B", "2026-09-12", 1)];
        let (vectors, when) = space(&two);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &Edits::default());
        assert!(threads.notes.is_empty(), "nothing placed yet");

        let (vectors, when) = space(&[two[0], two[1], ("01Z1", "2026-09-01", 9)]);
        assert!(threads.reconcile(&vectors, &when, &Edits::default()));
        assert_eq!(threads_of(&threads, &when), vec![vec!["01A", "01B"]]);
    }

    /// Five notes about one thing make the mean mostly that thing; with it
    /// taken off, two notes about anything else share not being about it.
    #[test]
    fn a_space_mostly_about_one_thing_does_not_tie_the_rest_together() {
        let notes = [
            ("01A", "2026-09-10", 1),
            ("01B", "2026-09-11", 1),
            ("01C", "2026-09-12", 1),
            ("01D", "2026-09-13", 1),
            ("01E", "2026-09-14", 1),
            // An apple, then Angular.
            ("01P", "2026-09-15", 2),
            ("01Q", "2026-09-16", 3),
        ];
        let (vectors, when) = space(&notes);

        let mut mean_only = Threads::default();
        let cuts = Cuts {
            lead: f32::NEG_INFINITY,
            ..CUTS
        };
        mean_only.reconcile_with(&vectors, &when, &Edits::default(), cuts);
        assert!(mean_only.of("01P").is_some(), "the old mistake");

        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &Edits::default());
        assert_eq!(
            threads_of(&threads, &when),
            vec![vec!["01A", "01B", "01C", "01D", "01E"]]
        );
    }

    /// Two threads of one thing, apart by more than the window, until a
    /// note between them draws one towards the other.
    fn bridged() -> (
        Vec<(&'static str, &'static str, usize)>,
        Vec<(&'static str, &'static str, usize)>,
    ) {
        let apart = with_others(&[
            ("01A", "2026-01-01", 1),
            ("01B", "2026-01-02", 1),
            ("01C", "2026-04-01", 1),
            ("01D", "2026-04-02", 1),
        ]);
        let mut between = apart.clone();
        between.push(("01E", "2026-02-15", 1));
        (apart, between)
    }

    #[test]
    fn threads_that_grow_towards_each_other_become_one() {
        let (apart, between) = bridged();
        let (vectors, when) = space(&apart);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &Edits::default());
        assert_eq!(
            threads_of(&threads, &when),
            vec![vec!["01A", "01B"], vec!["01C", "01D"]]
        );

        let (vectors, when) = space(&between);
        assert!(threads.reconcile(&vectors, &when, &Edits::default()));
        assert_eq!(
            threads_of(&threads, &when),
            vec![vec!["01A", "01B", "01E", "01C", "01D"]]
        );
        assert_eq!(threads.of("01D"), Some("01A"), "the older one goes on");
    }

    #[test]
    fn a_titled_thread_keeps_its_name_and_two_never_become_one() {
        let (apart, between) = bridged();
        let titled = |threads: &[&str]| Edits {
            titles: threads
                .iter()
                .map(|thread| (thread.to_string(), "Mine".to_string()))
                .collect(),
            ..Edits::default()
        };

        let one = titled(&["01C"]);
        let (vectors, when) = space(&apart);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &one);
        let (vectors, when) = space(&between);
        threads.reconcile(&vectors, &when, &one);
        assert_eq!(threads_of(&threads, &when).len(), 1);
        assert_eq!(threads.of("01A"), Some("01C"));

        let both = titled(&["01A", "01C"]);
        let (vectors, when) = space(&apart);
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &both);
        let (vectors, when) = space(&between);
        threads.reconcile(&vectors, &when, &both);
        assert_eq!(threads_of(&threads, &when).len(), 2);
    }

    #[test]
    fn a_note_without_a_day_waits_for_a_later_pass() {
        let notes = with_others(&[("01A", "2026-09-10", 1), ("01B", "2026-09-12", 1)]);
        let (vectors, mut when) = space(&notes);
        when.remove("01B");
        let mut threads = Threads::default();
        threads.reconcile(&vectors, &when, &Edits::default());
        assert!(threads_of(&threads, &when).is_empty());

        let (_, when) = space(&notes);
        assert!(threads.reconcile(&vectors, &when, &Edits::default()));
        assert_eq!(threads_of(&threads, &when), vec![vec!["01A", "01B"]]);
    }

    #[test]
    fn edits_round_trip_and_load_empty_when_none_are_saved() {
        let mut db = SpaceDb::in_memory().unwrap();
        assert_eq!(Edits::load(&db), Edits::default());

        let mut edits = Edits::default();
        edits.titles.insert("01A".into(), "Kitchen".into());
        edits.alone.insert("01D".into());
        edits.pinned.insert("01B".into(), "01A".into());
        edits
            .dismissed
            .insert("01E".into(), ["01E".to_string(), "01F".to_string()].into());
        edits.save(&mut db).unwrap();
        assert_eq!(Edits::load(&db), edits);

        // A save replaces what was saved.
        edits.titles.clear();
        edits.dismissed.clear();
        edits.save(&mut db).unwrap();
        assert_eq!(Edits::load(&db), edits);
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
        threads.reconcile(&vectors, &when, &Edits::default());
        let listed = threads.list(&when, &Edits::default());

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
        threads.reconcile(&vectors, &when, &Edits::default());
        let took = started.elapsed();
        eprintln!("placed {count} notes in {took:?}");
        if !cfg!(debug_assertions) {
            assert!(took.as_secs_f32() < 5.0, "took {took:?}");
        }
    }
}
