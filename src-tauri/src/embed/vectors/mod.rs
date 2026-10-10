//! The note embeddings search by meaning reads, held in memory and saved in
//! the `vectors` table of `space.db`.
//!
//! Derived like `search.db`: every vector can be made again from the
//! markdown, so vectors that are missing or from another model load as
//! empty and the notes are simply embedded again. `vectors.bin`, where they
//! were saved before, is read by `legacy`.

mod legacy;

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use super::math::{add, dot, from_le_bytes, normalize, to_le_bytes};
use crate::storage::index::IndexEntry;
use crate::storage::space_db::SpaceDb;
use crate::Result;

#[derive(Debug)]
pub struct Vectors {
    model_id: String,
    dims: usize,
    /// Note id to the body hash it was embedded from, and its unit vector.
    notes: HashMap<String, (String, Vec<f32>)>,
    /// Every note's vector added up, kept as notes come and go so that
    /// scoring against what is usual never adds them all up again. In f64:
    /// a note's vector is taken off when it changes or goes, and the
    /// rounding of years of that must not build up.
    sum: Vec<f64>,
    /// What a save has to write.
    unsaved: Unsaved,
}

#[derive(Debug)]
enum Unsaved {
    /// Every note, in place of whatever is saved: the vectors are new, or
    /// from another model.
    All,
    /// The notes stored or forgotten since the last save.
    Notes(HashSet<String>),
}

/// The same notes from the same model. The sum follows from the notes, and
/// adding them in another order can round it a hair differently.
impl PartialEq for Vectors {
    fn eq(&self, other: &Self) -> bool {
        self.model_id == other.model_id && self.dims == other.dims && self.notes == other.notes
    }
}

impl Vectors {
    pub fn new(model_id: &str, dims: usize) -> Self {
        Self {
            model_id: model_id.to_string(),
            dims,
            notes: HashMap::new(),
            sum: vec![0.0; dims],
            unsaved: Unsaved::All,
        }
    }

    /// The saved vectors if they came from this model at this size, else an
    /// empty store. Never an error: the vectors can always be made again.
    pub fn load(db: &SpaceDb, model_id: &str, dims: usize) -> Self {
        let saved = db.meta("vectors_model").as_deref() == Some(model_id)
            && db.meta("vectors_dims") == Some(dims.to_string());
        let loaded = if saved {
            Self::read(db.conn(), model_id, dims)
        } else {
            None
        };
        loaded.unwrap_or_else(|| Self::new(model_id, dims))
    }

    /// Every row, `None` if one is not a vector of this size. The sum is
    /// added up in id order, as `vectors.bin` held them, so every score
    /// comes out the same. Read in the table's own order, which is faster
    /// than going through the id index row by row.
    fn read(conn: &Connection, model_id: &str, dims: usize) -> Option<Self> {
        let mut statement = conn.prepare("SELECT id, hash, vector FROM vectors").ok()?;
        let mut rows = statement.query([]).ok()?;
        let mut vectors = Self::new(model_id, dims);
        while let Some(row) = rows.next().ok()? {
            let bytes = row.get_ref(2).ok()?.as_blob().ok()?;
            if bytes.len() != dims * 4 {
                return None;
            }
            let vector = from_le_bytes(bytes);
            vectors
                .notes
                .insert(row.get(0).ok()?, (row.get(1).ok()?, vector));
        }
        let mut ids: Vec<&String> = vectors.notes.keys().collect();
        ids.sort();
        for id in ids {
            add(&mut vectors.sum, &vectors.notes[id].1, 1.0);
        }
        vectors.unsaved = Unsaved::Notes(HashSet::new());
        Some(vectors)
    }

    /// Write what changed since the last save, all of it in one go.
    pub fn save(&mut self, db: &mut SpaceDb) -> Result<()> {
        db.transaction(|tx| self.write(tx))?;
        self.unsaved = Unsaved::Notes(HashSet::new());
        Ok(())
    }

    /// What `save` writes, in the caller's transaction.
    pub(crate) fn write(&self, tx: &Connection) -> Result<()> {
        let mut put = tx.prepare_cached(
            "INSERT OR REPLACE INTO vectors (id, hash, vector) VALUES (?1, ?2, ?3)",
        )?;
        let mut row = |id: &str| -> Result<()> {
            let (hash, vector) = &self.notes[id];
            put.execute(rusqlite::params![id, hash, to_le_bytes(vector)])?;
            Ok(())
        };
        match &self.unsaved {
            Unsaved::All => {
                tx.execute("DELETE FROM vectors", [])?;
                SpaceDb::set_meta(tx, "vectors_model", &self.model_id)?;
                SpaceDb::set_meta(tx, "vectors_dims", &self.dims.to_string())?;
                self.notes.keys().try_for_each(|id| row(id))
            }
            Unsaved::Notes(ids) => ids.iter().try_for_each(|id| {
                if self.notes.contains_key(id) {
                    row(id)
                } else {
                    tx.execute("DELETE FROM vectors WHERE id = ?1", [id])?;
                    Ok(())
                }
            }),
        }
    }

    /// Note that `id` was stored or forgotten.
    fn touch(&mut self, id: &str) {
        if let Unsaved::Notes(ids) = &mut self.unsaved {
            ids.insert(id.to_string());
        }
    }

    /// Whether these vectors came from this model at this size.
    pub fn is_from(&self, model_id: &str, dims: usize) -> bool {
        self.model_id == model_id && self.dims == dims
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    pub fn dims(&self) -> usize {
        self.dims
    }

    pub fn len(&self) -> usize {
        self.notes.len()
    }

    /// A note's unit vector and the body hash it was made from.
    pub fn get(&self, id: &str) -> Option<(&str, &[f32])> {
        self.notes
            .get(id)
            .map(|(hash, vector)| (hash.as_str(), vector.as_slice()))
    }

    /// Every note's vector added up, which says what is usual in the space.
    pub fn sum(&self) -> Vec<f32> {
        self.sum.iter().map(|&s| s as f32).collect()
    }

    /// Every note's id, body hash and unit vector, in no order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str, &[f32])> {
        self.notes
            .iter()
            .map(|(id, (hash, vector))| (id.as_str(), hash.as_str(), vector.as_slice()))
    }

    /// Notes with no vector yet or one made from an older body, in the order
    /// given.
    pub fn stale<'a>(
        &self,
        entries: impl IntoIterator<Item = &'a IndexEntry>,
    ) -> Vec<&'a IndexEntry> {
        entries
            .into_iter()
            .filter(|entry| {
                !matches!(self.notes.get(&entry.id), Some((hash, _)) if *hash == entry.hash)
            })
            .collect()
    }

    /// Forget notes that are gone. True when anything was dropped.
    pub fn retain(&mut self, present: &HashSet<String>) -> bool {
        let before = self.notes.len();
        let sum = &mut self.sum;
        let mut gone = Vec::new();
        self.notes.retain(|id, (_, vector)| {
            let keep = present.contains(id);
            if !keep {
                add(sum, vector, -1.0);
                gone.push(id.clone());
            }
            keep
        });
        for id in gone {
            self.touch(&id);
        }
        self.settle();
        self.notes.len() != before
    }

    /// Store `id`'s vector, embedded from the body `hash` names, at unit
    /// length in place of any it had. Refused when it is not as long as
    /// the store's vectors.
    pub fn insert(&mut self, id: String, hash: String, mut vector: Vec<f32>) -> Result<()> {
        if vector.len() != self.dims {
            return Err(format!(
                "expected a vector of {} dimensions, got {}",
                self.dims,
                vector.len()
            )
            .into());
        }
        normalize(&mut vector);
        add(&mut self.sum, &vector, 1.0);
        self.touch(&id);
        if let Some((_, old)) = self.notes.insert(id, (hash, vector)) {
            add(&mut self.sum, &old, -1.0);
        }
        Ok(())
    }

    /// An empty space adds up to nothing, whatever rounding was left over.
    fn settle(&mut self) {
        if self.notes.is_empty() {
            self.sum.fill(0.0);
        }
    }

    /// The `k` notes closest to the query with their cosine, best first.
    /// Equal scores go by id, so one question always picks the same notes.
    #[cfg(test)]
    pub fn top(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        if query.len() != self.dims {
            return Vec::new();
        }
        let mut query = query.to_vec();
        normalize(&mut query);

        let scored: Vec<(&String, f32)> = self
            .notes
            .iter()
            .map(|(id, (_, vector))| (id, dot(&query, vector)))
            .collect();
        ranked(scored, k)
    }

    /// Up to `k` notes that stand out for the query, best first, each with its
    /// lead: its cosine minus the mean cosine of every other note. Short
    /// queries score about the same against a whole space of notes by one
    /// person, and how high that level sits changes with the query, so no
    /// plain cosine cut works for all of them; the lead does.
    pub fn related(&self, query: &[f32], k: usize, min_lead: f32) -> Vec<(String, f32)> {
        if query.len() != self.dims || self.notes.len() < 2 {
            return Vec::new();
        }
        let mut query = query.to_vec();
        normalize(&mut query);

        let scores: Vec<(&String, f32)> = self
            .notes
            .iter()
            .map(|(id, (_, vector))| (id, dot(&query, vector)))
            .collect();
        let total: f32 = scores.iter().map(|(_, score)| score).sum();
        let others = (scores.len() - 1) as f32;
        let leads: Vec<(&String, f32)> = scores
            .into_iter()
            .map(|(id, score)| (id, score - (total - score) / others))
            .filter(|(_, lead)| *lead >= min_lead)
            .collect();
        ranked(leads, k)
    }

    /// Up to `k` other notes that score at least `min_score` against this
    /// one, best first. Empty for a note with no vector yet. No model runs:
    /// the note's vector is already here.
    ///
    /// The score is a cosine taken after removing the mean of the space's
    /// other vectors. Notes share a lot just by being short notes by one
    /// person, so plain cosines bunch up: two unrelated work notes score
    /// about as high as two notes on the same show, and a bland note comes up
    /// for everything. Once the mean is removed, only what a note has beyond
    /// the usual counts.
    ///
    /// The mean leaves out the two notes being compared. In a space of ten,
    /// two notes on the same thing would otherwise make a fifth of what is
    /// "usual", and taking it off would remove most of what they share. With
    /// a few dozen notes this changes next to nothing.
    pub fn similar(&self, id: &str, k: usize, min_score: f32) -> Vec<(String, f32)> {
        let Some((_, a)) = self.notes.get(id) else {
            return Vec::new();
        };
        self.closest(a, Some(id), k, min_score)
    }

    /// Up to `k` notes that score at least `min_score` against `query`, best
    /// first, scored as `similar` scores them. The query need not be stored:
    /// it can be a draft not saved yet. `exclude`, such as the note a draft
    /// is an edit of, is neither listed nor counted in the mean.
    pub fn closest(
        &self,
        query: &[f32],
        exclude: Option<&str>,
        k: usize,
        min_score: f32,
    ) -> Vec<(String, f32)> {
        if query.len() != self.dims {
            return Vec::new();
        }
        let mut a = query.to_vec();
        normalize(&mut a);
        let others: Vec<(&String, &Vec<f32>)> = self
            .notes
            .iter()
            .filter(|(id, _)| Some(id.as_str()) != exclude)
            .map(|(id, (_, vector))| (id, vector))
            .collect();
        // Nothing is left to say what is usual.
        if others.len() < 2 {
            return Vec::new();
        }
        let rest = (others.len() - 1) as f32;
        let mut sum = self.sum();
        if let Some((_, left_out)) = exclude.and_then(|id| self.notes.get(id)) {
            for (s, x) in sum.iter_mut().zip(left_out) {
                *s -= x;
            }
        }

        // Expanded rather than centred copies, with m = (S - b) / rest, S the
        // sum of every note but the one left out: (a - m)·(b - m) is
        // a·b - a·m - b·m + m·m, and |a - m|² is a·a - 2 a·m + m·m, each dot
        // with m written through S.
        let ss = dot(&sum, &sum);
        let aa = dot(&a, &a);
        let a_s = dot(&a, &sum);

        let scored: Vec<(&String, f32)> = others
            .into_iter()
            .filter_map(|(other, b)| {
                let (ab, bb, b_s) = (dot(&a, b), dot(b, b), dot(b, &sum));
                let am = (a_s - ab) / rest;
                let bm = (b_s - bb) / rest;
                let mm = (ss - 2.0 * b_s + bb) / (rest * rest);
                let norm =
                    (aa - 2.0 * am + mm).max(0.0).sqrt() * (bb - 2.0 * bm + mm).max(0.0).sqrt();
                // A note that is the mean has no direction of its own left.
                (norm > 0.0).then(|| (other, (ab - am - bm + mm) / norm))
            })
            .filter(|(_, score)| *score >= min_score)
            .collect();
        ranked(scored, k)
    }
}

/// Best first, equal scores by id, cut to `k`.
fn ranked(mut scored: Vec<(&String, f32)>, k: usize) -> Vec<(String, f32)> {
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    scored.truncate(k);
    scored
        .into_iter()
        .map(|(id, score)| (id.clone(), score))
        .collect()
}

#[cfg(test)]
mod tests;
