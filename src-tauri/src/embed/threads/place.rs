//! Placing the notes of one scope in threads, SPEC 6.4: each note joins
//! the thread it scores best against, or starts one with a note on its own,
//! and threads that grow close become one.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use chrono::NaiveDate;

use super::score::{earliest, latest, Group, Part, Usual};
use super::{thread_id, Cuts, Edits, Placed, Scope, When, GENERAL, MIN_NOTES};
use crate::embed::vectors::Vectors;

/// Where a note goes.
enum Choice {
    Join(String),
    Pair(String),
    /// Where the user put it, which need not exist yet.
    Put(String),
}

/// Place the notes of one scope, `members`, as `Threads::reconcile` says,
/// scored against what the scope's notes add up to. True when anything
/// changed.
pub(super) fn place(
    placed: &mut Scope,
    scope: &str,
    members: &BTreeSet<&str>,
    vectors: &Vectors,
    when: &HashMap<String, When>,
    edits: &Edits,
    cuts: Cuts,
) -> bool {
    let cuts = if scope == GENERAL {
        cuts
    } else {
        Cuts {
            lead: cuts.scope_lead,
            ..cuts
        }
    };
    let changed = forget_stale(placed, scope, members, vectors, edits);

    let mut todo: Vec<(&When, &str)> = members
        .iter()
        .filter(|id| !placed.contains_key(**id))
        .filter_map(|id| Some((when.get(*id)?, *id)))
        .collect();
    if todo.is_empty() || members.len() < MIN_NOTES {
        return changed;
    }
    // Notes put in a thread by the user first, so the others can join.
    todo.sort_by_key(|(written, id)| (edits.put(scope, id).is_none(), *written, *id));

    let mut placing = Placing::new(placed, scope, members, vectors, when, edits, cuts);
    // Threads that formed or grew, which may take in a note on its own.
    let mut touched: BTreeSet<String> = BTreeSet::new();
    for (written, id) in todo {
        placing.place_note(placed, written, id, &mut touched);
    }
    placing.settle(placed, touched);
    true
}

/// Forget where the notes were placed that are to be placed again: those
/// gone from the scope, written again since, taken out of threads or let
/// back in, or put in another thread by the user, and the last note of a
/// thread down to one. True when any was.
fn forget_stale(
    placed: &mut Scope,
    scope: &str,
    members: &BTreeSet<&str>,
    vectors: &Vectors,
    edits: &Edits,
) -> bool {
    let alone = &edits.alone;
    let before = placed.len();
    placed.retain(|id, placed| {
        members.contains(id.as_str())
            && vectors.get(id).is_some_and(|(hash, _)| hash == placed.hash)
            && placed.out == alone.contains(id)
            && edits
                .put(scope, id)
                .is_none_or(|thread| placed.thread.as_ref() == Some(thread))
    });
    // A thread down to one note is no thread: that note is placed again.
    let mut sizes: HashMap<String, usize> = HashMap::new();
    for thread in placed.values().filter_map(|placed| placed.thread.clone()) {
        *sizes.entry(thread).or_default() += 1;
    }
    placed.retain(|_, placed| {
        placed
            .thread
            .as_ref()
            .is_none_or(|thread| sizes[thread] > 1)
    });
    placed.len() != before
}

/// One scope's notes as they are placed: the threads so far, and the
/// notes on their own that a note may start a thread with.
struct Placing<'a> {
    scope: &'a str,
    vectors: &'a Vectors,
    when: &'a HashMap<String, When>,
    edits: &'a Edits,
    cuts: Cuts,
    usual: Usual,
    /// Ordered, so equal scores always pick the same thread.
    groups: BTreeMap<String, Group>,
    /// By day.
    single: BTreeSet<(NaiveDate, String)>,
}

impl<'a> Placing<'a> {
    /// The threads and the notes on their own as `placed` has them.
    fn new(
        placed: &Scope,
        scope: &'a str,
        members: &BTreeSet<&str>,
        vectors: &'a Vectors,
        when: &'a HashMap<String, When>,
        edits: &'a Edits,
        cuts: Cuts,
    ) -> Self {
        let usual = Usual::of_members(vectors, members);
        let mut groups: BTreeMap<String, Group> = BTreeMap::new();
        let mut single: BTreeSet<(NaiveDate, String)> = BTreeSet::new();
        for (id, placed) in placed.iter() {
            let (Some(written), Some((_, vector))) = (when.get(id), vectors.get(id)) else {
                continue;
            };
            match &placed.thread {
                Some(thread) => groups
                    .entry(thread.clone())
                    .or_insert_with(|| Group::new(vectors.dims(), written.date))
                    .add(id, vector, written.date, &usual),
                None if !edits.alone.contains(id) => {
                    single.insert((written.date, id.clone()));
                }
                None => {}
            }
        }
        Self {
            scope,
            vectors,
            when,
            edits,
            cuts,
            usual,
            groups,
            single,
        }
    }

    /// Place note `id`, written at `written`, and add the thread it went
    /// into to `touched`.
    fn place_note(
        &mut self,
        placed: &mut Scope,
        written: &When,
        id: &str,
        touched: &mut BTreeSet<String>,
    ) {
        let (vectors, edits) = (self.vectors, self.edits);
        let alone = &edits.alone;
        let (hash, vector) = vectors.get(id).expect("listed from the vectors");
        let choice = if alone.contains(id) {
            None
        } else if let Some(thread) = edits.put(self.scope, id) {
            Some(Choice::Put(thread.clone()))
        } else {
            let note = self.usual.note(id, vector);
            self.best(&note, written.date)
        };
        let thread = match choice {
            Some(Choice::Put(thread)) => {
                self.groups
                    .entry(thread.clone())
                    .or_insert_with(|| Group::new(vectors.dims(), written.date))
                    .add(id, vector, written.date, &self.usual);
                touched.insert(thread.clone());
                Some(thread)
            }
            Some(Choice::Join(thread)) => {
                if let Some(group) = self.groups.get_mut(&thread) {
                    group.add(id, vector, written.date, &self.usual);
                }
                touched.insert(thread.clone());
                Some(thread)
            }
            Some(Choice::Pair(other)) => {
                let (Some(their), Some((_, theirs))) = (self.when.get(&other), vectors.get(&other))
                else {
                    return;
                };
                // Named after the earlier of the two, unless a thread
                // already goes by that name.
                let mut names = [other.as_str(), id];
                if (written, id) < (their, other.as_str()) {
                    names.reverse();
                }
                let thread = names
                    .iter()
                    .map(|name| thread_id(self.scope, name))
                    .chain((2..).map(|n| thread_id(self.scope, &format!("{}-{n}", names[0]))))
                    .find(|name| !self.groups.contains_key(name))
                    .expect("a free name");
                let mut group = Group::new(vectors.dims(), their.date);
                group.add(&other, theirs, their.date, &self.usual);
                group.add(id, vector, written.date, &self.usual);
                self.groups.insert(thread.clone(), group);
                self.single.remove(&(their.date, other.clone()));
                if let Some(placed) = placed.get_mut(&other) {
                    placed.thread = Some(thread.clone());
                }
                touched.insert(thread.clone());
                Some(thread)
            }
            None => {
                if !alone.contains(id) {
                    self.single.insert((written.date, id.to_string()));
                }
                None
            }
        };
        let note = Placed {
            hash: hash.to_string(),
            thread,
            out: alone.contains(id),
        };
        placed.insert(id.to_string(), note);
    }

    /// A note on its own was placed before the thread it fits formed, or
    /// before the thread grew towards it: it joins now. A thread that grew
    /// towards another becomes one with it. Either may draw a thread nearer
    /// others in turn, so this goes on until no thread of `touched` moves.
    fn settle(&mut self, placed: &mut Scope, mut touched: BTreeSet<String>) {
        let (vectors, edits) = (self.vectors, self.edits);
        let window = self.cuts.window;
        let kept = edits.kept();
        while !touched.is_empty() {
            let near: BTreeSet<(NaiveDate, String)> = touched
                .iter()
                .filter_map(|thread| self.groups.get(thread))
                .flat_map(|group| {
                    let last = latest(group.last, window);
                    self.single
                        .range((earliest(group.first, window), String::new())..)
                        .take_while(move |(day, _)| *day <= last)
                        .cloned()
                })
                .collect();
            let grown = std::mem::take(&mut touched);
            for (day, id) in near {
                let Some((_, vector)) = vectors.get(&id) else {
                    continue;
                };
                let note = self.usual.note(&id, vector);
                let Some((_, thread)) = self.best_thread(&note, day) else {
                    continue;
                };
                if let Some(group) = self.groups.get_mut(&thread) {
                    group.add(&id, vector, day, &self.usual);
                }
                self.single.remove(&(day, id.clone()));
                if let Some(placed) = placed.get_mut(&id) {
                    placed.thread = Some(thread.clone());
                }
                touched.insert(thread);
            }
            for thread in grown {
                let Some(other) = self.closest_thread(&thread, &kept) else {
                    continue;
                };
                // The user's goes on, else the older one.
                let older =
                    |a: &str, b: &str| (self.groups[a].first, a) < (self.groups[b].first, b);
                let (keep, gone) = if kept.contains(other.as_str())
                    || (!kept.contains(thread.as_str()) && older(&other, &thread))
                {
                    (other, thread)
                } else {
                    (thread, other)
                };
                let absorbed = self.groups.remove(&gone).expect("a thread just scored");
                if let Some(group) = self.groups.get_mut(&keep) {
                    group.absorb(absorbed, &self.usual);
                }
                for placed in placed.values_mut() {
                    if placed.thread.as_deref() == Some(gone.as_str()) {
                        placed.thread = Some(keep.clone());
                    }
                }
                touched.remove(&gone);
                touched.insert(keep);
            }
        }
    }

    /// Where a note of `date` goes: the thread it scores best against, or
    /// the note on its own it scores best against, among those near enough
    /// in time that clear their threshold. `None` keeps it on its own.
    fn best(&self, note: &Part, date: NaiveDate) -> Option<Choice> {
        let cuts = self.cuts;
        let mut best: Option<(f32, Choice)> = self
            .best_thread(note, date)
            .map(|(score, thread)| (score, Choice::Join(thread)));
        let last = latest(date, cuts.window);
        for (day, other) in self
            .single
            .range((earliest(date, cuts.window), String::new())..)
        {
            if *day > last {
                break;
            }
            let Some((_, theirs)) = self.vectors.get(other) else {
                continue;
            };
            let Some(score) = self
                .usual
                .score(note, &self.usual.note(other, theirs), cuts.lead)
            else {
                continue;
            };
            if score >= cuts.pair && best.as_ref().is_none_or(|(top, _)| score > *top) {
                best = Some((score, Choice::Pair(other.clone())));
            }
        }
        best.map(|(_, choice)| choice)
    }

    /// The thread near `date` a note scores best against, if it clears the
    /// cut to join, with its score.
    fn best_thread(&self, note: &Part, date: NaiveDate) -> Option<(f32, String)> {
        let cuts = self.cuts;
        let mut best: Option<(f32, String)> = None;
        let near = self
            .groups
            .iter()
            .filter(|(_, group)| group.near(date, cuts.window));
        for (thread, group) in near {
            let Some(score) = self.usual.score(note, &group.part(), cuts.lead) else {
                continue;
            };
            if score >= cuts.join && best.as_ref().is_none_or(|(top, _)| score > *top) {
                best = Some((score, thread.clone()));
            }
        }
        best
    }

    /// The thread near `thread` in time that it would become one with,
    /// unless both are the user's, `kept`: of the two, every note of the
    /// smaller clears the cut to join the larger, as it would have had it
    /// come after. Two threads' averages are not compared: averaging
    /// smooths away what sets each apart, and threads would chain into one.
    fn closest_thread(&self, thread: &str, kept: &HashSet<&str>) -> Option<String> {
        let (cuts, usual) = (self.cuts, &self.usual);
        let group = self.groups.get(thread)?;
        let mut best: Option<(f32, &String)> = None;
        let near = self
            .groups
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
                let score = self
                    .vectors
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
}
