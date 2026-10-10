//! How a note scores against a thread or another note, SPEC 6.4: the
//! cosine of their averages once what every other note shares is taken
//! off, and each standing out for the other.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{Days, NaiveDate};

use super::MIN_NOTES;
use crate::embed::math::{add, add_into, dot};
use crate::embed::vectors::Vectors;

/// A thread while notes are placed: its notes, what they add up to, and the
/// days they span.
pub(super) struct Group {
    pub(super) notes: Vec<String>,
    sum: Vec<f32>,
    /// sum · sum and sum · the space's sum, kept as the thread grows.
    own: f32,
    all: f32,
    pub(super) first: NaiveDate,
    pub(super) last: NaiveDate,
}

impl Group {
    pub(super) fn new(dims: usize, date: NaiveDate) -> Self {
        Self {
            notes: Vec::new(),
            sum: vec![0.0; dims],
            own: 0.0,
            all: 0.0,
            first: date,
            last: date,
        }
    }

    pub(super) fn add(&mut self, id: &str, vector: &[f32], date: NaiveDate, usual: &Usual) {
        add_into(&mut self.sum, vector);
        self.notes.push(id.to_string());
        self.own = dot(&self.sum, &self.sum);
        self.all = dot(&self.sum, &usual.sum);
        self.first = self.first.min(date);
        self.last = self.last.max(date);
    }

    /// Take in the notes of `other`.
    pub(super) fn absorb(&mut self, other: Group, usual: &Usual) {
        add_into(&mut self.sum, &other.sum);
        self.notes.extend(other.notes);
        self.own = dot(&self.sum, &self.sum);
        self.all = dot(&self.sum, &usual.sum);
        self.first = self.first.min(other.first);
        self.last = self.last.max(other.last);
    }

    pub(super) fn part(&self) -> Part<'_> {
        Part {
            sum: &self.sum,
            count: self.notes.len(),
            own: self.own,
            all: self.all,
        }
    }

    /// Whether a note of `date` lies within `window` days of this thread's.
    pub(super) fn near(&self, date: NaiveDate, window: u64) -> bool {
        earliest(self.first, window) <= date && date <= latest(self.last, window)
    }

    /// Whether some note of `other` lies within `window` days of this
    /// thread's.
    pub(super) fn meets(&self, other: &Group, window: u64) -> bool {
        earliest(self.first, window) <= other.last && other.first <= latest(self.last, window)
    }
}

/// The first day `window` days before `date`.
pub(super) fn earliest(date: NaiveDate, window: u64) -> NaiveDate {
    date.checked_sub_days(Days::new(window))
        .unwrap_or(NaiveDate::MIN)
}

/// The last day `window` days after `date`.
pub(super) fn latest(date: NaiveDate, window: u64) -> NaiveDate {
    date.checked_add_days(Days::new(window))
        .unwrap_or(NaiveDate::MAX)
}

/// A note or a thread as it is scored: what it adds up to, how many notes
/// that is, and the two dot products of its sum that placing a note leaves
/// alone, so that each comparison costs only one more.
pub(super) struct Part<'a> {
    sum: &'a [f32],
    count: usize,
    /// sum · sum
    own: f32,
    /// sum · the space's sum
    all: f32,
}

/// What every note of the space adds up to, which says what is usual, and
/// each note's dot products with itself and that sum.
pub(super) struct Usual {
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

    /// What the notes `members` add up to, as `of` for the whole space,
    /// which it is when they are every note.
    pub(super) fn of_members(vectors: &Vectors, members: &BTreeSet<&str>) -> Self {
        if members.len() == vectors.len() {
            return Self::of(vectors);
        }
        let mut total = vec![0.0f64; vectors.dims()];
        for (_, vector) in members.iter().filter_map(|id| vectors.get(id)) {
            add(&mut total, vector, 1.0);
        }
        let sum: Vec<f32> = total.into_iter().map(|t| t as f32).collect();
        let dots: HashMap<String, (f32, f32)> = members
            .iter()
            .filter_map(|id| {
                let (_, vector) = vectors.get(id)?;
                Some((id.to_string(), (dot(vector, vector), dot(vector, &sum))))
            })
            .collect();
        Self {
            ss: dot(&sum, &sum),
            sum,
            count: dots.len(),
            dots,
        }
    }

    /// What the space adds up to with `text` written into it, as `DRAFT`,
    /// and `exclude`, the note it is an edit of, left out.
    fn with_text(vectors: &Vectors, text: &[f32], exclude: Option<&str>) -> Self {
        let mut total: Vec<f64> = vectors.sum().iter().map(|s| f64::from(*s)).collect();
        let left_out = exclude.and_then(|id| vectors.get(id));
        for (i, t) in total.iter_mut().enumerate() {
            *t += f64::from(text[i]);
            if let Some((_, vector)) = left_out {
                *t -= f64::from(vector[i]);
            }
        }
        let sum: Vec<f32> = total.into_iter().map(|t| t as f32).collect();
        let mut dots: HashMap<String, (f32, f32)> = vectors
            .iter()
            .filter(|(id, _, _)| Some(*id) != exclude)
            .map(|(id, _, vector)| (id.to_string(), (dot(vector, vector), dot(vector, &sum))))
            .collect();
        dots.insert(DRAFT.to_string(), (dot(text, text), dot(text, &sum)));
        Self {
            ss: dot(&sum, &sum),
            sum,
            count: dots.len(),
            dots,
        }
    }

    /// A note of the space as it is scored.
    pub(super) fn note<'a>(&self, id: &str, vector: &'a [f32]) -> Part<'a> {
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
    pub(super) fn score(&self, a: &Part, b: &Part, lead: f32) -> Option<f32> {
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

/// What a text not saved yet stands for among the vectors, as `Usual`
/// keys it.
const DRAFT: &str = "\u{0}draft";

/// The name whose notes a text not saved yet fits best (SPEC 3.10), with
/// its score, when it clears `cut`: the text scored against each name's
/// notes as a note against a thread, the space's mean taken off, and each
/// standing out for the other by `lead`. `names` holds each name's notes;
/// only names with `MIN_NOTES` of them are weighed, as fewer say little.
/// `exclude` is the note the text is an edit of, which counts for nothing.
pub fn closest_name(
    vectors: &Vectors,
    names: &BTreeMap<String, Vec<String>>,
    text: &[f32],
    exclude: Option<&str>,
    cut: f32,
    lead: f32,
) -> Option<(String, f32)> {
    let usual = Usual::with_text(vectors, text, exclude);
    let draft = usual.note(DRAFT, text);
    let mut best: Option<(String, f32)> = None;
    for (name, notes) in names {
        let mut group = Group::new(vectors.dims(), NaiveDate::MIN);
        for id in notes.iter().filter(|id| Some(id.as_str()) != exclude) {
            if let Some((_, vector)) = vectors.get(id) {
                group.add(id, vector, NaiveDate::MIN, &usual);
            }
        }
        if group.notes.len() < MIN_NOTES {
            continue;
        }
        let Some(score) = usual.score(&draft, &group.part(), lead) else {
            continue;
        };
        if score >= cut && best.as_ref().is_none_or(|(_, top)| score > *top) {
            best = Some((name.clone(), score));
        }
    }
    best
}
