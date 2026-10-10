use super::legacy::{put_str, MAGIC, VERSION};
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
