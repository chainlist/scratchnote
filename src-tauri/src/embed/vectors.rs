//! `vectors.bin`, the note embeddings a chat searches to pick its notes.
//!
//! Derived like `index.jsonl`: every vector can be made again from the
//! markdown, so a file that is missing, damaged or from another model loads
//! as empty and the notes are simply embedded again.
//!
//! Little endian throughout. The header is `SNVB`, a u32 version, the model
//! id, the dims as u32 and the count as u32. Each note follows as its id, its
//! body hash and `dims` f32 values. A string is a u32 byte length then UTF-8.

use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::normalize;
use crate::storage::index::IndexEntry;

const MAGIC: &[u8; 4] = b"SNVB";
const VERSION: u32 = 1;

#[derive(Debug, PartialEq)]
pub struct Vectors {
    model_id: String,
    dims: usize,
    /// Note id to the body hash it was embedded from, and its unit vector.
    notes: HashMap<String, (String, Vec<f32>)>,
}

pub fn vectors_path(root: &Path) -> PathBuf {
    root.join(".scratchnote").join("vectors.bin")
}

impl Vectors {
    pub fn new(model_id: &str, dims: usize) -> Self {
        Self {
            model_id: model_id.to_string(),
            dims,
            notes: HashMap::new(),
        }
    }

    /// The saved vectors if they came from this model at this size, else an
    /// empty store. Never an error: the file can always be rebuilt.
    pub fn load(path: &Path, model_id: &str, dims: usize) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| Self::decode(&bytes))
            .filter(|saved| saved.is_from(model_id, dims))
            .unwrap_or_else(|| Self::new(model_id, dims))
    }

    /// Whether these vectors came from this model at this size.
    pub fn is_from(&self, model_id: &str, dims: usize) -> bool {
        self.model_id == model_id && self.dims == dims
    }

    /// Temp file, fsync, rename, as the writer does, so a crash mid-save
    /// leaves the previous file whole. Written here rather than through the
    /// writer because the watcher never looks at it and the writer only
    /// takes text.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("bin.tmp");
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(&self.encode())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)
    }

    /// Notes with no vector yet or one made from an older body, in the order
    /// given. An empty body has nothing to embed.
    pub fn stale<'a>(
        &self,
        entries: impl IntoIterator<Item = &'a IndexEntry>,
    ) -> Vec<&'a IndexEntry> {
        entries
            .into_iter()
            .filter(|entry| !entry.body.trim().is_empty())
            .filter(|entry| {
                !matches!(self.notes.get(&entry.id), Some((hash, _)) if *hash == entry.hash)
            })
            .collect()
    }

    /// Forget notes that are gone. True when anything was dropped.
    pub fn retain(&mut self, present: &HashSet<String>) -> bool {
        let before = self.notes.len();
        self.notes.retain(|id, _| present.contains(id));
        self.notes.len() != before
    }

    pub fn insert(&mut self, id: String, hash: String, mut vector: Vec<f32>) -> Result<(), String> {
        if vector.len() != self.dims {
            return Err(format!(
                "expected a vector of {} dimensions, got {}",
                self.dims,
                vector.len()
            ));
        }
        normalize(&mut vector);
        self.notes.insert(id, (hash, vector));
        Ok(())
    }

    /// The `k` notes closest to the query with their cosine, best first.
    /// Equal scores go by id, so one question always picks the same notes.
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

    /// Up to `k` other notes that score at least `min_score` against this
    /// one, best first. Empty for a note with no vector yet. No model runs:
    /// the note's vector is already here.
    ///
    /// The score is a cosine taken after removing the mean of the space's
    /// vectors. Notes share a lot just by being short notes by one person,
    /// so plain cosines bunch up: two unrelated work notes score about as
    /// high as two notes on the same show, and a bland note comes up for
    /// everything. Once the mean is removed, only what a note has beyond the
    /// usual counts.
    pub fn similar(&self, id: &str, k: usize, min_score: f32) -> Vec<(String, f32)> {
        let Some((_, note)) = self.notes.get(id) else {
            return Vec::new();
        };
        let count = self.notes.len() as f32;
        let mut mean = vec![0.0; self.dims];
        for (_, vector) in self.notes.values() {
            for (m, x) in mean.iter_mut().zip(vector) {
                *m += x / count;
            }
        }

        // Expanded rather than centred copies: (a - m)·(b - m) is
        // a·b - a·m - b·m + m·m, and |a - m|² is a·a - 2 a·m + m·m.
        let mm = dot(&mean, &mean);
        let centred_norm = |v: &[f32], vm: f32| (dot(v, v) - 2.0 * vm + mm).max(0.0).sqrt();
        let note_m = dot(note, &mean);
        let note_norm = centred_norm(note, note_m);

        let scored: Vec<(&String, f32)> = self
            .notes
            .iter()
            .filter(|(other, _)| other.as_str() != id)
            .filter_map(|(other, (_, vector))| {
                let other_m = dot(vector, &mean);
                let norm = note_norm * centred_norm(vector, other_m);
                // A note that is the mean has no direction of its own left.
                (norm > 0.0).then(|| (other, (dot(note, vector) - note_m - other_m + mm) / norm))
            })
            .filter(|(_, score)| *score >= min_score)
            .collect();
        ranked(scored, k)
    }

    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.notes.len() * (self.dims * 4 + 48));
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        put_str(&mut out, &self.model_id);
        out.extend_from_slice(&(self.dims as u32).to_le_bytes());
        out.extend_from_slice(&(self.notes.len() as u32).to_le_bytes());
        for (id, (hash, vector)) in &self.notes {
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
        for _ in 0..count {
            let id = reader.string()?;
            let hash = reader.string()?;
            let vector = reader
                .take(dims.checked_mul(4)?)?
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            notes.insert(id, (hash, vector));
        }

        reader.0.is_empty().then_some(Self {
            model_id,
            dims,
            notes,
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

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

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
    use crate::storage::daily_file::{body_hash, Note, Status};

    fn entry(id: &str, body: &str) -> IndexEntry {
        IndexEntry::from(&Note {
            id: id.to_string(),
            date: "2026-09-22".to_string(),
            time: "08:00".to_string(),
            file: "notes/2026/2026-09-22.md".to_string(),
            subject: None,
            summary: None,
            tags: Vec::new(),
            status: Status::Done,
            hash: body_hash(body),
            body: body.to_string(),
        })
    }

    fn scratch_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("scratchnote-vectors-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
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
        let root = scratch_root("round-trip");
        let path = vectors_path(&root);
        let vectors = sample();

        vectors.save(&path).unwrap();
        assert_eq!(Vectors::load(&path, "model-3", 3), vectors);
        assert!(!path.with_extension("bin.tmp").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_from_another_model_or_size_loads_empty() {
        let root = scratch_root("other-model");
        let path = vectors_path(&root);
        sample().save(&path).unwrap();

        assert_eq!(
            Vectors::load(&path, "model-4", 3),
            Vectors::new("model-4", 3)
        );
        assert_eq!(
            Vectors::load(&path, "model-3", 4),
            Vectors::new("model-3", 4)
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_or_corrupt_file_loads_empty() {
        let root = scratch_root("corrupt");
        let path = vectors_path(&root);
        assert_eq!(Vectors::load(&path, "m", 3), Vectors::new("m", 3));

        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"not vectors at all").unwrap();
        assert_eq!(Vectors::load(&path, "m", 3), Vectors::new("m", 3));

        let _ = std::fs::remove_dir_all(&root);
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
    fn stale_lists_new_and_edited_notes_but_not_unchanged_or_empty_ones() {
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
            entry("01D", "  \n"),
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
