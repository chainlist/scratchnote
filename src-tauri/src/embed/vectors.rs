//! The note embeddings search by meaning reads, held in memory and saved in
//! the `vectors` table of `space.db`.
//!
//! Derived like `index.jsonl`: every vector can be made again from the
//! markdown, so vectors that are missing or from another model load as
//! empty and the notes are simply embedded again.
//!
//! `vectors.bin`, the file they were saved in before, is read once when
//! `space.db` is made. Little endian throughout. The header is `SNVB`, a u32
//! version, the model id, the dims as u32 and the count as u32. Each note
//! follows, in id order, as its id, its body hash and `dims` f32 values. A
//! string is a u32 byte length then UTF-8.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::Connection;

use super::math::{add, dot, from_le_bytes, normalize, to_le_bytes};
use crate::storage::index::IndexEntry;
use crate::storage::space_db::SpaceDb;
use crate::Result;

const MAGIC: &[u8; 4] = b"SNVB";
const VERSION: u32 = 1;

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

    /// The vectors `vectors.bin` holds, `None` for anything but a whole,
    /// well-formed file.
    pub fn read_bin(path: &Path) -> Option<Self> {
        Self::decode(&std::fs::read(path).ok()?)
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

    /// The file `read_bin` reads, as the app saved it before `space.db`.
    #[cfg(test)]
    pub fn write_bin(&self, path: &Path) {
        std::fs::write(path, self.encode()).unwrap();
    }

    #[cfg(test)]
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.notes.len() * (self.dims * 4 + 48));
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        put_str(&mut out, &self.model_id);
        out.extend_from_slice(&(self.dims as u32).to_le_bytes());
        out.extend_from_slice(&(self.notes.len() as u32).to_le_bytes());
        let mut notes: Vec<_> = self.notes.iter().collect();
        notes.sort_by(|a, b| a.0.cmp(b.0));
        for (id, (hash, vector)) in notes {
            put_str(&mut out, id);
            put_str(&mut out, hash);
            for x in vector {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        out
    }

    /// `None` for anything but a whole, well-formed file: every length is
    /// checked against what is left, and nothing may trail the last note.
    fn decode(bytes: &[u8]) -> Option<Self> {
        let mut reader = Reader(bytes);
        if reader.take(4)? != MAGIC || reader.u32()? != VERSION {
            return None;
        }
        let model_id = reader.string()?;
        let dims = reader.u32()? as usize;
        let count = reader.u32()?;

        // Not sized from `count`: a damaged count must not allocate.
        let mut notes = HashMap::new();
        let mut sum = vec![0.0; dims];
        for _ in 0..count {
            let id = reader.string()?;
            let hash = reader.string()?;
            let vector = from_le_bytes(reader.take(dims.checked_mul(4)?)?);
            add(&mut sum, &vector, 1.0);
            if let Some((_, old)) = notes.insert(id, (hash, vector)) {
                add(&mut sum, &old, -1.0);
            }
        }

        reader.0.is_empty().then_some(Self {
            model_id,
            dims,
            notes,
            sum,
            unsaved: Unsaved::All,
        })
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
fn put_str(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
    out.extend_from_slice(text.as_bytes());
}

/// What is left of the file to decode.
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if n > self.0.len() {
            return None;
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Some(head)
    }

    fn u32(&mut self) -> Option<u32> {
        self.take(4)?.try_into().ok().map(u32::from_le_bytes)
    }

    fn string(&mut self) -> Option<String> {
        let len = self.u32()? as usize;
        String::from_utf8(self.take(len)?.to_vec()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::{Embedder, StubEmbedder};
    use crate::storage::daily_file::{body_hash, Kind, Note};

    fn entry(id: &str, body: &str) -> IndexEntry {
        IndexEntry::from(&Note {
            id: id.to_string(),
            date: "2026-09-22".to_string(),
            time: "08:00".to_string(),
            file: "notes/2026/2026-09-22.md".to_string(),
            subject: None,
            hash: body_hash(body),
            ahead_off: false,
            body: body.to_string(),
            kind: Kind::Note,
            on: None,
            missing: false,
        })
    }

    fn sample() -> Vectors {
        let mut vectors = Vectors::new("model-3", 3);
        vectors
            .insert("01A".into(), "aaaa".into(), vec![1.0, 2.0, 2.0])
            .unwrap();
        vectors
            .insert("01B".into(), "bbbb".into(), vec![0.0, 0.0, -4.0])
            .unwrap();
        vectors
    }

    fn ids(hits: &[(String, f32)]) -> Vec<&str> {
        hits.iter().map(|(id, _)| id.as_str()).collect()
    }

    #[test]
    fn saves_and_loads_back_the_same_vectors() {
        let mut db = SpaceDb::in_memory().unwrap();
        let mut vectors = sample();
        vectors.save(&mut db).unwrap();
        assert_eq!(Vectors::load(&db, "model-3", 3), vectors);
    }

    #[test]
    fn the_kept_sum_follows_notes_edited_removed_and_loaded() {
        let fresh = |vectors: &Vectors| {
            let mut sum = vec![0.0f32; vectors.dims()];
            for (_, _, vector) in vectors.iter() {
                for (s, x) in sum.iter_mut().zip(vector) {
                    *s += x;
                }
            }
            sum
        };
        let close = |a: Vec<f32>, b: Vec<f32>| a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-6);

        let mut vectors = sample();
        vectors
            .insert("01C".into(), "cccc".into(), vec![3.0, 0.0, 4.0])
            .unwrap();
        // An edit replaces the old vector rather than adding to it.
        vectors
            .insert("01A".into(), "aaab".into(), vec![0.0, 1.0, 0.0])
            .unwrap();
        assert!(close(vectors.sum(), fresh(&vectors)));

        vectors.retain(&["01A".to_string(), "01C".to_string()].into());
        assert!(close(vectors.sum(), fresh(&vectors)));

        let loaded = Vectors::decode(&vectors.encode()).unwrap();
        assert!(close(loaded.sum(), fresh(&vectors)));
        let mut db = SpaceDb::in_memory().unwrap();
        vectors.save(&mut db).unwrap();
        assert!(close(
            Vectors::load(&db, "model-3", 3).sum(),
            fresh(&vectors)
        ));

        vectors.retain(&HashSet::new());
        assert_eq!(vectors.sum(), vec![0.0; 3]);
    }

    #[test]
    fn vectors_from_another_model_or_size_load_empty() {
        let mut db = SpaceDb::in_memory().unwrap();
        assert_eq!(Vectors::load(&db, "model-3", 3), Vectors::new("model-3", 3));
        sample().save(&mut db).unwrap();

        assert_eq!(Vectors::load(&db, "model-4", 3), Vectors::new("model-4", 3));
        assert_eq!(Vectors::load(&db, "model-3", 4), Vectors::new("model-3", 4));
    }

    #[test]
    fn a_missing_or_corrupt_vectors_bin_reads_as_none() {
        let dir = std::env::temp_dir().join("scratchnote-vectors-corrupt");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("vectors.bin");
        assert!(Vectors::read_bin(&path).is_none());

        std::fs::write(&path, b"not vectors at all").unwrap();
        assert!(Vectors::read_bin(&path).is_none());

        sample().write_bin(&path);
        assert_eq!(Vectors::read_bin(&path), Some(sample()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_truncation_or_trailing_byte_is_rejected() {
        let bytes = sample().encode();
        assert!(Vectors::decode(&bytes).is_some());
        for end in 0..bytes.len() {
            assert!(Vectors::decode(&bytes[..end]).is_none(), "cut at {end}");
        }
        let mut longer = bytes;
        longer.push(0);
        assert!(Vectors::decode(&longer).is_none());
    }

    #[test]
    fn a_save_writes_only_the_notes_stored_or_forgotten_since() {
        let mut db = SpaceDb::in_memory().unwrap();
        let mut vectors = Vectors::new("m", 2);
        for i in 0..100 {
            vectors
                .insert(format!("01A{i:03}"), "h".into(), vec![1.0, i as f32])
                .unwrap();
        }
        vectors.save(&mut db).unwrap();

        let rows = |db: &SpaceDb| db.conn().total_changes();
        let before = rows(&db);
        vectors.save(&mut db).unwrap();
        assert_eq!(rows(&db), before, "nothing to write");

        vectors
            .insert("01A007".into(), "h2".into(), vec![0.0, 1.0])
            .unwrap();
        vectors
            .insert("01B000".into(), "h".into(), vec![0.0, 1.0])
            .unwrap();
        let present = vectors
            .iter()
            .map(|(id, _, _)| id.to_string())
            .filter(|id| id != "01A050")
            .collect();
        vectors.retain(&present);
        vectors.save(&mut db).unwrap();
        assert_eq!(rows(&db), before + 3, "one edited, one new, one gone");
        assert_eq!(Vectors::load(&db, "m", 2), vectors);
    }

    #[test]
    fn a_huge_count_or_length_fails_without_allocating_it() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        put_str(&mut bytes, "m");
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(Vectors::decode(&bytes).is_none());
    }

    #[test]
    fn stale_lists_new_and_edited_notes_but_not_unchanged_ones() {
        let mut vectors = Vectors::new("m", 3);
        vectors
            .insert("01A".into(), body_hash("unchanged"), vec![1.0, 0.0, 0.0])
            .unwrap();
        vectors
            .insert("01B".into(), body_hash("before"), vec![1.0, 0.0, 0.0])
            .unwrap();

        let entries = [
            entry("01A", "unchanged"),
            entry("01B", "after"),
            entry("01C", "brand new"),
        ];
        let stale: Vec<&str> = vectors
            .stale(&entries)
            .iter()
            .map(|e| e.id.as_str())
            .collect();
        assert_eq!(stale, vec!["01B", "01C"]);
    }

    #[test]
    fn retain_forgets_deleted_notes() {
        let mut vectors = sample();
        let present: HashSet<String> = ["01A".to_string()].into();

        assert!(vectors.retain(&present));
        assert_eq!(ids(&vectors.top(&[1.0, 0.0, 0.0], 10)), vec!["01A"]);
        assert!(!vectors.retain(&present));
    }

    #[test]
    fn insert_stores_unit_vectors_and_rejects_the_wrong_size() {
        let mut vectors = sample();
        assert!(vectors
            .insert("01C".into(), "cccc".into(), vec![1.0, 0.0])
            .is_err());

        let (_, stored) = &vectors.notes["01A"];
        let norm: f32 = stored.iter().map(|x| x * x).sum();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn top_ranks_the_note_about_the_question_first() {
        let embedder = StubEmbedder;
        let mut vectors = Vectors::new(embedder.model_id(), embedder.dims());
        for (id, body) in [
            ("01A", "Bought coffee beans at the market"),
            ("01B", "Deploy kubernetes to staging on Friday"),
            ("01C", "Grandma's birthday dinner on Sunday"),
        ] {
            let vector = embedder.embed_document(body).unwrap();
            vectors.insert(id.into(), body_hash(body), vector).unwrap();
        }

        let query = embedder.embed_query("kubernetes deploy").unwrap();
        let hits = vectors.top(&query, 3);
        assert_eq!(hits[0].0, "01B");
        assert!(hits[0].1 > hits[1].1);
        assert_eq!(vectors.top(&query, 1).len(), 1);
    }

    #[test]
    fn equal_scores_are_ordered_by_id() {
        let mut vectors = Vectors::new("m", 2);
        for id in ["01C", "01A", "01B"] {
            vectors
                .insert(id.into(), "h".into(), vec![1.0, 1.0])
                .unwrap();
        }
        assert_eq!(
            ids(&vectors.top(&[1.0, 0.0], 10)),
            vec!["01A", "01B", "01C"]
        );
    }

    #[test]
    fn similar_leaves_out_the_note_itself_and_what_is_only_shared_by_all() {
        // Every note leans on the first axis, as real notes share a lot.
        // 01A and 01B are also about the same thing; 01C and 01D are not.
        let mut vectors = Vectors::new("m", 4);
        for (id, vector) in [
            ("01A", vec![1.0, 1.0, 0.0, 0.0]),
            ("01B", vec![1.0, 0.9, 0.1, 0.0]),
            ("01C", vec![1.0, 0.0, 1.0, 0.0]),
            ("01D", vec![1.0, 0.0, 0.0, 1.0]),
        ] {
            vectors.insert(id.into(), "h".into(), vector).unwrap();
        }

        // A plain cosine puts 01C and 01D at 0.5 from 01A, on the shared
        // axis alone.
        let plain = vectors.top(&vectors.notes["01A"].1, 4);
        assert!(plain.iter().all(|(_, score)| *score >= 0.5));

        let hits = vectors.similar("01A", 5, 0.28);
        assert_eq!(ids(&hits), vec!["01B"]);
        assert!(hits[0].1 > 0.9);
        assert_eq!(ids(&vectors.similar("01A", 2, -1.0)).len(), 2);
        assert!(vectors.similar("01Z", 5, -1.0).is_empty());
    }

    #[test]
    fn similar_finds_the_pair_in_a_space_of_a_few_notes() {
        // With five notes, 01A and 01B weigh two fifths of the mean, so taking
        // it off removes most of what they share.
        let mut vectors = Vectors::new("m", 7);
        for (id, vector) in [
            ("01A", vec![1.0, 0.5, 1.0, 0.0, 0.0, 0.0, 0.0]),
            ("01B", vec![1.0, 0.5, 0.0, 1.0, 0.0, 0.0, 0.0]),
            ("01C", vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
            ("01D", vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
            ("01E", vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
        ] {
            vectors.insert(id.into(), "h".into(), vector).unwrap();
        }
        assert_eq!(ids(&vectors.similar("01A", 5, 0.28)), vec!["01B"]);
        assert_eq!(ids(&vectors.similar("01B", 5, 0.28)), vec!["01A"]);
    }

    #[test]
    fn closest_scores_a_draft_as_similar_scores_the_note_it_would_be() {
        let mut vectors = Vectors::new("m", 4);
        for (id, vector) in [
            ("01A", vec![1.0, 1.0, 0.0, 0.0]),
            ("01B", vec![1.0, 0.9, 0.1, 0.0]),
            ("01C", vec![1.0, 0.0, 1.0, 0.0]),
            ("01D", vec![1.0, 0.0, 0.0, 1.0]),
        ] {
            vectors.insert(id.into(), "h".into(), vector).unwrap();
        }

        // A draft of 01A, with 01A left out as the note being edited, finds
        // what Similar notes finds for 01A.
        let draft = [2.0, 2.0, 0.0, 0.0];
        let found = vectors.closest(&draft, Some("01A"), 5, 0.28);
        let similar = vectors.similar("01A", 5, 0.28);
        assert_eq!(ids(&found), ids(&similar));
        assert_eq!(ids(&found), vec!["01B"]);
        assert!((found[0].1 - similar[0].1).abs() < 1e-5);

        // A new draft has nothing left out, so 01A is closest to it.
        assert_eq!(ids(&vectors.closest(&draft, None, 1, 0.28)), vec!["01A"]);
        assert!(vectors.closest(&[1.0, 0.0], None, 5, -1.0).is_empty());

        // Two notes are not enough to say what is usual.
        let mut two = Vectors::new("m", 4);
        for id in ["01A", "01B"] {
            two.insert(id.into(), "h".into(), vec![1.0, 1.0, 0.0, 0.0])
                .unwrap();
        }
        assert!(two.closest(&draft, Some("01A"), 5, -1.0).is_empty());
    }

    #[test]
    fn related_keeps_the_notes_that_stand_out_for_the_query() {
        // Every note leans on the first axis; only 01A is about the query.
        let mut vectors = Vectors::new("m", 4);
        for (id, vector) in [
            ("01A", vec![1.0, 1.0, 0.0, 0.0]),
            ("01B", vec![1.0, 0.0, 1.0, 0.0]),
            ("01C", vec![1.0, 0.0, 0.0, 1.0]),
        ] {
            vectors.insert(id.into(), "h".into(), vector).unwrap();
        }
        let hits = vectors.related(&[1.0, 1.0, 0.0, 0.0], 5, 0.15);
        assert_eq!(ids(&hits), vec!["01A"]);
        assert!((hits[0].1 - 0.5).abs() < 1e-5, "1.0 against a mean of 0.5");

        // Close to every note alike, so close to none in particular.
        assert!(vectors.related(&[1.0, 0.0, 0.0, 0.0], 5, 0.15).is_empty());

        let mut alone = Vectors::new("m", 4);
        alone
            .insert("01A".into(), "h".into(), vec![1.0, 1.0, 0.0, 0.0])
            .unwrap();
        assert!(alone.related(&[1.0, 1.0, 0.0, 0.0], 5, -1.0).is_empty());
    }

    /// Brute force is only fine while it stays this fast at the scale retrieval
    /// is for. Only a gate in release, like search's own timing test:
    /// `cargo test --release top_is_fast`.
    #[test]
    fn top_is_fast_enough_for_ten_thousand_notes() {
        const DIMS: usize = 1024;
        // A fixed xorshift, so the vectors are random but the same every run.
        let mut state: u32 = 0x9e37_79b9;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as f32 / u32::MAX as f32 - 0.5
        };

        let mut vectors = Vectors::new("m", DIMS);
        for i in 0..10_000 {
            let vector: Vec<f32> = (0..DIMS).map(|_| next()).collect();
            vectors
                .insert(format!("{i:05}"), "h".into(), vector)
                .unwrap();
        }
        let query: Vec<f32> = (0..DIMS).map(|_| next()).collect();

        let started = std::time::Instant::now();
        let hits = vectors.top(&query, 20);
        let took = started.elapsed();

        assert_eq!(hits.len(), 20);
        eprintln!("ranked 10,000 vectors in {took:?}");
        if !cfg!(debug_assertions) {
            assert!(took.as_millis() < 50, "took {took:?}");
        }
    }
}
