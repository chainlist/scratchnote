//! How long a space's vectors, threads and thread edits take to load and
//! save, and what they give, on a copy of a real space. Only `Store` knows
//! where they are kept, so the rest runs unchanged across a change of
//! storage, and two runs compare both the timings and the results.
//!
//! Ignored, and only fair in release:
//! `SCRATCHNOTE_BENCH_SPACE=<space folder> SCRATCHNOTE_BENCH_OUT=<file.json>
//! cargo test --release bench_storage -- --ignored --nocapture`.
//! The space is copied first, never written to.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use serde_json::{json, Value};

use super::threads::{when_written, Edits, Threads, When};
use super::vectors::Vectors;
use crate::spaces::Space;
use crate::storage::space_db::{space_db_path, SpaceDb};

/// Where the vectors, threads and edits are kept.
struct Store {
    root: PathBuf,
    db: SpaceDb,
}

impl Store {
    fn open(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            db: SpaceDb::open(root).unwrap(),
        }
    }

    fn load_vectors(&self) -> Vectors {
        let model = self.db.meta("vectors_model").unwrap();
        let dims = self.db.meta("vectors_dims").unwrap().parse().unwrap();
        Vectors::load(&self.db, &model, dims)
    }

    fn save_vectors(&mut self, vectors: &mut Vectors) {
        vectors.save(&mut self.db).unwrap();
    }

    fn load_threads(&self) -> Threads {
        Threads::load(&self.db)
    }

    fn save_threads(&mut self, threads: &mut Threads) {
        threads.save(&mut self.db).unwrap();
    }

    fn load_edits(&self) -> Edits {
        Edits::load(&self.db)
    }

    fn save_edits(&mut self, edits: &Edits) {
        edits.save(&mut self.db).unwrap();
    }

    /// Bytes on disk, the write-ahead log included.
    fn size(&self) -> u64 {
        let path = space_db_path(&self.root);
        let mut wal = path.clone().into_os_string();
        wal.push("-wal");
        [path, PathBuf::from(wal)]
            .iter()
            .filter_map(|path| std::fs::metadata(path).ok())
            .map(|meta| meta.len())
            .sum()
    }
}

// Nothing below knows where things are kept.

const RUNS: usize = 15;
const SAVE_EVERY: usize = 500;

#[derive(Default)]
struct Timings(Vec<(String, Vec<Duration>)>);

impl Timings {
    fn add(&mut self, name: &str, took: Duration) {
        match self.0.iter_mut().find(|(n, _)| n == name) {
            Some((_, all)) => all.push(took),
            None => self.0.push((name.to_string(), vec![took])),
        }
    }

    fn report(&self) -> Value {
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        println!(
            "{:<34} {:>6} {:>10} {:>10} {:>10}",
            "", "runs", "median", "min", "max"
        );
        let mut out = serde_json::Map::new();
        for (name, all) in &self.0 {
            let mut sorted = all.clone();
            sorted.sort();
            let median = sorted[sorted.len() / 2];
            println!(
                "{:<34} {:>6} {:>8.2}ms {:>8.2}ms {:>8.2}ms",
                name,
                all.len(),
                ms(median),
                ms(sorted[0]),
                ms(sorted[sorted.len() - 1])
            );
            out.insert(
                name.clone(),
                json!({
                    "runs": all.len(),
                    "median_ms": ms(median),
                    "min_ms": ms(sorted[0]),
                    "max_ms": ms(sorted[sorted.len() - 1]),
                    "total_ms": ms(all.iter().sum()),
                }),
            );
        }
        Value::Object(out)
    }
}

fn timed<T>(timings: &mut Timings, name: &str, run: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let out = run();
    timings.add(name, started.elapsed());
    out
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// FNV-1a over every vector's id, hash and bits, in id order: equal only
/// for the very same vectors.
fn checksum(vectors: &Vectors) -> String {
    let mut notes: Vec<_> = vectors.iter().collect();
    notes.sort_by(|a, b| a.0.cmp(b.0));
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    };
    for (id, hash, vector) in notes {
        eat(id.as_bytes());
        eat(hash.as_bytes());
        for x in vector {
            eat(&x.to_bits().to_le_bytes());
        }
    }
    format!("{h:016x}")
}

/// What the views get from the vectors, threads and edits. Scores are to be
/// compared within a tolerance: `related` adds up cosines in hash map order,
/// which moves the last bits from run to run whatever the storage. The
/// checksum is exact.
fn digest(
    vectors: &Vectors,
    threads: &Threads,
    edits: &Edits,
    when: &HashMap<String, When>,
) -> Value {
    let mut ids: Vec<&str> = vectors.iter().map(|(id, _, _)| id).collect();
    ids.sort();
    let sampled: Vec<&str> = ids
        .iter()
        .step_by(ids.len().div_ceil(200).max(1))
        .copied()
        .collect();
    let similar: BTreeMap<&str, _> = sampled
        .iter()
        .map(|id| (*id, vectors.similar(id, 8, 0.28)))
        .collect();
    let related: BTreeMap<&str, _> = sampled
        .iter()
        .step_by(10)
        .map(|id| {
            let (_, query) = vectors.get(id).unwrap();
            let mut hits = vectors.related(query, usize::MAX, 0.15);
            hits.truncate(20);
            (*id, hits)
        })
        .collect();
    let placed: BTreeMap<&str, Option<&str>> = ids.iter().map(|id| (*id, threads.of(id))).collect();
    json!({
        "notes": vectors.len(),
        "model": vectors.model_id(),
        "dims": vectors.dims(),
        "vectors_checksum": checksum(vectors),
        "similar": similar,
        "related": related,
        "placed": placed,
        "threads": serde_json::to_value(threads.list(when, edits)).unwrap(),
        "edits": serde_json::to_value(edits).unwrap(),
    })
}

#[test]
#[ignore]
fn bench_storage() {
    let source =
        PathBuf::from(std::env::var("SCRATCHNOTE_BENCH_SPACE").expect("SCRATCHNOTE_BENCH_SPACE"));
    let root = std::env::temp_dir().join(format!("scratchnote-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    copy_dir(&source, &root);
    let mut timings = Timings::default();

    // Before the space opens, which would otherwise be first to read in
    // whatever the store reads in when it is new.
    let store = timed(&mut timings, "open store", || Store::open(&root));
    drop(store);
    let mut store = timed(&mut timings, "reopen store", || Store::open(&root));
    let size_before = store.size();
    let (space, _) = Space::open("bench", root.clone(), Arc::new(tokio::sync::Notify::new()));
    let mut when = when_written(&space.index.read().unwrap());
    for _ in 0..RUNS {
        timed(&mut timings, "load vectors", || store.load_vectors());
        timed(&mut timings, "load threads", || store.load_threads());
        timed(&mut timings, "load edits", || store.load_edits());
    }
    let mut vectors = store.load_vectors();
    let mut threads = store.load_threads();
    let mut edits = store.load_edits();
    println!(
        "{} notes, {} dims, {} on disk before",
        vectors.len(),
        vectors.dims(),
        size_before
    );

    // As loaded: placing again must change nothing.
    let unchanged = !threads.reconcile(&vectors, &when, &edits);
    let loaded = digest(&vectors, &threads, &edits, &when);

    let mut ids: Vec<String> = vectors.iter().map(|(id, _, _)| id.to_string()).collect();
    ids.sort();
    let last_day = when
        .values()
        .map(|w| w.date)
        .max()
        .unwrap_or(NaiveDate::MIN);
    for run in 0..RUNS {
        // A note edited: a new vector for it, then placed again.
        let id = ids[(run * 37) % ids.len()].clone();
        let mut vector = vectors.get(&id).unwrap().1.to_vec();
        let at = run % vector.len();
        vector[at] += 0.05;
        vectors
            .insert(id, format!("bench-edit-{run}"), vector)
            .unwrap();
        timed(&mut timings, "save vectors, one note edited", || {
            store.save_vectors(&mut vectors)
        });
        timed(&mut timings, "place threads, one note edited", || {
            threads.reconcile(&vectors, &when, &edits)
        });
        timed(&mut timings, "save threads, one note edited", || {
            store.save_threads(&mut threads)
        });

        // A note written: its id sorts last, as a new one's does.
        let id = format!("ZZBENCH{run:03}");
        // Not a copy of another note's vector, which `related` would score
        // a tie with it, ordered by the last bits.
        let mut vector = vectors
            .get(&ids[(run * 53) % ids.len()])
            .unwrap()
            .1
            .to_vec();
        let at = (run + 1) % vector.len();
        vector[at] += 0.05;
        vectors
            .insert(id.clone(), format!("bench-new-{run}"), vector)
            .unwrap();
        when.insert(
            id,
            When {
                date: last_day,
                time: format!("23:{run:02}"),
            },
        );
        timed(&mut timings, "save vectors, one note new", || {
            store.save_vectors(&mut vectors)
        });
        timed(&mut timings, "place threads, one note new", || {
            threads.reconcile(&vectors, &when, &edits)
        });
        timed(&mut timings, "save threads, one note new", || {
            store.save_threads(&mut threads)
        });

        // A note deleted.
        let gone = ids[(run * 71 + 5) % ids.len()].clone();
        let present: HashSet<String> = vectors
            .iter()
            .map(|(id, _, _)| id.to_string())
            .filter(|id| *id != gone)
            .collect();
        vectors.retain(&present);
        when.remove(&gone);
        timed(&mut timings, "save vectors, one note deleted", || {
            store.save_vectors(&mut vectors)
        });
        timed(&mut timings, "place threads, one note deleted", || {
            threads.reconcile(&vectors, &when, &edits)
        });
        timed(&mut timings, "save threads, one note deleted", || {
            store.save_threads(&mut threads)
        });

        // A thread renamed, as the command does it: read, change, write.
        let thread = threads.list(&when, &edits).first().map(|t| t.id.clone());
        timed(&mut timings, "rename a thread (load+save edits)", || {
            let mut fresh = store.load_edits();
            if let Some(thread) = thread {
                fresh.titles.insert(thread, format!("Bench {run}"));
            }
            store.save_edits(&fresh);
            edits = fresh;
        });
    }

    // Everything read back from where it was saved gives what memory had.
    let in_memory = digest(&vectors, &threads, &edits, &when);
    let mut reloaded_vectors = store.load_vectors();
    let reloaded_threads = store.load_threads();
    let reloaded_edits = store.load_edits();
    let reloaded = digest(&reloaded_vectors, &reloaded_threads, &reloaded_edits, &when);
    let round_trip = in_memory == reloaded;
    let size_after = store.size();

    // A first pass: every note embedded into an empty store, saved every
    // `SAVE_EVERY` notes and at the end, as the embed task does.
    let order: Vec<String> = space
        .index
        .read()
        .unwrap()
        .entries()
        .map(|e| e.id.clone())
        .filter(|id| vectors.get(id).is_some())
        .collect();
    let mut fresh = Vectors::new(vectors.model_id(), vectors.dims());
    let started = Instant::now();
    let mut saving = Duration::ZERO;
    for (n, id) in order.iter().enumerate() {
        let (hash, vector) = vectors.get(id).unwrap();
        fresh
            .insert(id.clone(), hash.to_string(), vector.to_vec())
            .unwrap();
        if (n + 1) % SAVE_EVERY == 0 {
            let at = Instant::now();
            store.save_vectors(&mut fresh);
            saving += at.elapsed();
        }
    }
    let at = Instant::now();
    store.save_vectors(&mut fresh);
    saving += at.elapsed();
    timings.add("first pass, all saves", saving);
    timings.add("first pass, inserts and saves", started.elapsed());
    reloaded_vectors = store.load_vectors();
    let first_pass_round_trip = checksum(&reloaded_vectors) == checksum(&fresh);

    println!();
    let report = timings.report();
    println!();
    println!("placing the loaded threads again changed nothing: {unchanged}");
    println!("everything read back equals memory, scores exactly: {round_trip}");
    println!("first pass read back equals memory: {first_pass_round_trip}");
    println!("on disk: {size_before} bytes before, {size_after} after the edits");

    if let Ok(out) = std::env::var("SCRATCHNOTE_BENCH_OUT") {
        let result = json!({
            "space": source.file_name().unwrap().to_string_lossy(),
            "timings": report,
            "unchanged_on_load": unchanged,
            "round_trip": round_trip,
            "first_pass_round_trip": first_pass_round_trip,
            "bytes_before": size_before,
            "bytes_after": size_after,
            "loaded": loaded,
            "after_edits_in_memory": in_memory,
            "after_edits": reloaded,
        });
        std::fs::write(out, serde_json::to_string_pretty(&result).unwrap()).unwrap();
    }
    drop(space);
    let _ = std::fs::remove_dir_all(&root);
}
