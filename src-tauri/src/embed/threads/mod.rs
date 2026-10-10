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
//! Threads are found within a scope: the notes mentioning one name, such as
//! `@ProjectA` (SPEC 3.10), or the general one, of the notes mentioning
//! none. A note mentioning two names is placed in each one's threads. Each
//! scope is placed on its own, against its own notes' mean, so what all of
//! a project's notes share is taken off and its subjects stand apart: the
//! threads of `@ProjectA` are its last 404 in production, its performance.
//! A thread of a mention's scope goes by `@key:note`, one of the general
//! scope by the note it started with, as before there were scopes.
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

mod edits;
mod place;
mod score;
mod store;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::vectors::Vectors;
use crate::storage::index::Index;
pub use edits::Edits;
use place::place;
pub use score::closest_name;
use score::{Group, Usual};
use store::Saved;

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
    /// The same among the notes of one name. They all share the name's
    /// subject, so their cosines sit closer together and a note leads its
    /// own subject's notes by less. On the made-up notes of `@Atlas` and
    /// `@Marie`, notes on one subject led each other by 0.09 to 0.27,
    /// notes on two by 0.06 at most.
    pub scope_lead: f32,
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
    scope_lead: 0.08,
    window: 60,
};

/// Bumped when the way notes are placed changes, so they are placed again.
const VERSION: u32 = 3;

/// The scope of the notes that mention no name.
pub const GENERAL: &str = "";

/// The scope a thread was found in, by its id.
pub fn scope_of(thread: &str) -> &str {
    thread
        .strip_prefix('@')
        .and_then(|rest| rest.split_once(':'))
        .map_or(GENERAL, |(key, _)| key)
}

/// The id of a thread of `scope` named after `note`.
pub fn thread_id(scope: &str, note: &str) -> String {
    if scope == GENERAL {
        note.to_string()
    } else {
        format!("@{scope}:{note}")
    }
}

/// The names each note mentions, by note, as `mentions.rs` keys them. A
/// note left out mentions none.
pub type Mentioned = HashMap<String, Vec<String>>;

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

/// Where each note of a scope was placed, by note.
type Scope = BTreeMap<String, Placed>;

/// Where every note of the space was placed, by scope.
#[derive(Debug, Default, Clone)]
pub struct Threads {
    version: u32,
    /// The model the vectors placed from came from.
    model: String,
    scopes: BTreeMap<String, Scope>,
    /// The placements as last loaded or saved, so a save writes only the
    /// notes placed since. `None` until they are, which writes them all.
    saved: Option<Saved>,
}

/// The same placements, however much of them is saved.
impl PartialEq for Threads {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version && self.model == other.model && self.scopes == other.scopes
    }
}

/// A thread as the views show it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Thread {
    /// The id of the note it started with, which it keeps, after `@key:`
    /// in the scope of a name mentioned.
    pub id: String,
    /// The key of the name whose notes it was found among, or `None` in the
    /// general scope.
    pub scope: Option<String>,
    /// That name as typed, which the commands fill in from the notes.
    pub mention: Option<String>,
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
    /// The days of its notes, each once, oldest first.
    pub days: Vec<String>,
}

impl Threads {
    /// A thread a note is in, if any: the general scope's first.
    #[cfg(test)]
    pub fn of(&self, id: &str) -> Option<&str> {
        self.scopes
            .values()
            .find_map(|notes| notes.get(id)?.thread.as_deref())
    }

    /// The thread a note is in within `scope`, if any.
    pub fn in_scope(&self, scope: &str, id: &str) -> Option<&str> {
        self.scopes.get(scope)?.get(id)?.thread.as_deref()
    }

    /// Place every note of `vectors` not placed yet, oldest first, and place
    /// again those written again since, and those the user took out of
    /// threads, let back in or put in a thread. A note the user put in a
    /// thread goes there first, whatever it scores, so the notes like it
    /// join. Notes gone are forgotten. A note `when` does not know is left
    /// for a later pass. Two threads of the user's never become one. Every
    /// note is in the general scope here: `reconcile_mentioned` places them
    /// by the names they mention. True when anything changed.
    #[cfg(test)]
    pub fn reconcile(
        &mut self,
        vectors: &Vectors,
        when: &HashMap<String, When>,
        edits: &Edits,
    ) -> bool {
        self.reconcile_with(vectors, when, edits, &Mentioned::new(), CUTS)
    }

    /// `reconcile`, each note in the scope of each name it mentions, or in
    /// the general one when it mentions none, and in any scope the user put
    /// it in a thread of.
    pub fn reconcile_mentioned(
        &mut self,
        vectors: &Vectors,
        when: &HashMap<String, When>,
        edits: &Edits,
        mentioned: &Mentioned,
    ) -> bool {
        self.reconcile_with(vectors, when, edits, mentioned, CUTS)
    }

    /// `reconcile_mentioned`, with the lines drawn at `cuts`.
    pub fn reconcile_with(
        &mut self,
        vectors: &Vectors,
        when: &HashMap<String, When>,
        edits: &Edits,
        mentioned: &Mentioned,
        cuts: Cuts,
    ) -> bool {
        let mut changed = false;
        if self.version != VERSION || self.model != vectors.model_id() {
            self.version = VERSION;
            self.model = vectors.model_id().to_string();
            self.scopes.clear();
            changed = true;
        }

        let mut members: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for (id, _, _) in vectors.iter() {
            match mentioned.get(id).filter(|keys| !keys.is_empty()) {
                Some(keys) => {
                    for key in keys {
                        members.entry(key.as_str()).or_default().insert(id);
                    }
                }
                None => {
                    members.entry(GENERAL).or_default().insert(id);
                }
            }
        }
        for (scope, note, _) in edits.puts() {
            if vectors.get(note).is_some() {
                members.entry(scope).or_default().insert(note);
            }
        }

        // A scope no note is in any more is forgotten.
        let before = self.scopes.len();
        self.scopes
            .retain(|scope, _| members.contains_key(scope.as_str()));
        changed |= self.scopes.len() != before;
        for (scope, notes) in &members {
            let placed = self.scopes.entry(scope.to_string()).or_default();
            changed |= place(placed, scope, notes, vectors, when, edits, cuts);
        }
        self.scopes.retain(|_, placed| !placed.is_empty());
        changed
    }

    /// Every thread, its notes oldest first, with the titles the user gave,
    /// but for suggestions the user dismissed that hold no other note since.
    pub fn list(&self, when: &HashMap<String, When>, edits: &Edits) -> Vec<Thread> {
        let mut members: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for ((_, id), placed) in flat(&self.scopes) {
            if let (Some(thread), true) = (&placed.thread, when.contains_key(id)) {
                members.entry(thread.as_str()).or_default().push(id);
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
        flat(&self.scopes)
            .filter_map(|(_, placed)| placed.thread.clone())
            .collect()
    }

    /// The threads a note could be put in, the one it fits best first,
    /// whatever their days and however low it scores: the user chooses.
    /// Those of every scope it is placed in, each scored against its own
    /// scope. Not one it is in.
    pub fn ranked_for(&self, id: &str, vectors: &Vectors) -> Vec<String> {
        let Some((_, vector)) = vectors.get(id) else {
            return Vec::new();
        };
        let mut ranked: Vec<(f32, &str)> = Vec::new();
        for (scope, notes) in self
            .scopes
            .iter()
            .filter(|(_, notes)| notes.contains_key(id))
        {
            let members: BTreeSet<&str> = notes.keys().map(String::as_str).collect();
            let usual = Usual::of_members(vectors, &members);
            let mut groups: BTreeMap<&str, Group> = BTreeMap::new();
            let mine = self.in_scope(scope, id);
            for (note, placed) in notes {
                let (Some(thread), Some((_, theirs))) =
                    (placed.thread.as_deref(), vectors.get(note))
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
            ranked.extend(
                groups
                    .iter()
                    .filter(|(_, group)| group.notes.len() > 1)
                    .map(|(thread, group)| {
                        let score = usual
                            .score(&note, &group.part(), f32::NEG_INFINITY)
                            .unwrap_or(f32::NEG_INFINITY);
                        (score, *thread)
                    }),
            );
        }
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
            .scopes
            .get(scope_of(id))?
            .iter()
            .filter(|(note, placed)| {
                placed.thread.as_deref() == Some(id) && when.contains_key(*note)
            })
            .map(|(note, _)| note.as_str())
            .collect();
        shown(id, notes, when, edits, &edits.kept())
    }
}

/// Every placement, in scope then note order.
fn flat(scopes: &BTreeMap<String, Scope>) -> impl Iterator<Item = ((&str, &str), &Placed)> {
    scopes.iter().flat_map(|(scope, notes)| {
        notes
            .iter()
            .map(move |(id, placed)| ((scope.as_str(), id.as_str()), placed))
    })
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
    let mut days: Vec<String> = notes.iter().map(|note| day(note)).collect();
    days.dedup();
    let scope = scope_of(id);
    Some(Thread {
        id: id.to_string(),
        scope: (scope != GENERAL).then(|| scope.to_string()),
        mention: None,
        named: title.is_some(),
        title,
        kept: kept.contains(id),
        since: days[0].clone(),
        until: days[days.len() - 1].clone(),
        days,
        notes: notes.into_iter().map(str::to_string).collect(),
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod speed;
