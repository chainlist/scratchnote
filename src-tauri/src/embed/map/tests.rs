use super::layout::{closest, principal, pull_and_push, Random, Settle, LINKS};
use super::*;
use std::collections::HashSet;

/// Notes about `topics` things, `each` per thing, ids in time order: a
/// shared axis plus the thing's own axis and a little noise.
fn topics(topics: usize, each: usize, dims: usize) -> Vectors {
    let mut vectors = Vectors::new("m", dims);
    let mut state: u32 = 12345;
    let mut noise = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state as f32 / u32::MAX as f32 - 0.5) * 0.3
    };
    for t in 0..topics {
        for e in 0..each {
            let mut v: Vec<f32> = (0..dims).map(|_| noise()).collect();
            v[0] += 1.0;
            v[1 + t] += 1.0;
            vectors
                .insert(format!("{e:03}{t:02}"), "h".into(), v)
                .unwrap();
        }
    }
    vectors
}

type Slots = (
    Mutex<Option<Vectors>>,
    Mutex<Option<Map>>,
    Mutex<Option<SpaceDb>>,
);

fn slots(vectors: Vectors) -> Slots {
    (
        Mutex::new(Some(vectors)),
        Mutex::new(None),
        Mutex::new(Some(SpaceDb::in_memory().unwrap())),
    )
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// The share of notes whose nearest note on the map is about the same
/// topic.
fn nearest_same_topic(map: &Map, topics: usize, each: usize) -> f32 {
    let notes: Vec<(usize, [f32; 2])> = (0..topics)
        .flat_map(|t| (0..each).map(move |e| (t, e)))
        .map(|(t, e)| (t, map.get(&format!("{e:03}{t:02}")).unwrap()))
        .collect();
    let same = notes
        .iter()
        .enumerate()
        .filter(|(i, (topic, at))| {
            let nearest = notes
                .iter()
                .enumerate()
                .filter(|(j, _)| j != i)
                .min_by(|a, b| distance(*at, a.1 .1).total_cmp(&distance(*at, b.1 .1)))
                .unwrap();
            nearest.1 .0 == *topic
        })
        .count();
    same as f32 / notes.len() as f32
}

fn saved(db: &Mutex<Option<SpaceDb>>) -> Map {
    Map::load(db.lock().unwrap().as_ref().unwrap())
}

fn places_of(map: &Map) -> HashMap<String, (String, [f32; 2])> {
    map.taken()
        .map(|(slot, id, hash)| (id.to_string(), (hash.to_string(), map.at[slot])))
        .collect()
}

fn places(map: &Mutex<Option<Map>>) -> HashMap<String, (String, [f32; 2])> {
    places_of(map.lock().unwrap().as_ref().unwrap())
}

/// A note's closest notes, by id.
fn near_ids(map: &Map, id: &str) -> Vec<String> {
    map.near[map.slots[id]]
        .iter()
        .map(|&(_, to)| map.id(to).to_string())
        .collect()
}

fn unmoved(
    before: &HashMap<String, (String, [f32; 2])>,
    after: &HashMap<String, (String, [f32; 2])>,
) -> usize {
    before
        .iter()
        .filter(|(id, (_, at))| after.get(*id).is_some_and(|(_, now)| now == at))
        .count()
}

#[test]
fn a_layout_gathers_each_topic_apart_from_the_others_and_is_saved() {
    let (vectors, map, db) = slots(topics(4, 12, 16));
    assert!(reconcile(&vectors, &map, &db, HashMap::new).unwrap());
    {
        let laid = map.lock().unwrap();
        let laid = laid.as_ref().unwrap();
        assert_eq!(laid.len(), 48);
        let same = nearest_same_topic(laid, 4, 12);
        assert!(same >= 0.95, "{same} of notes are nearest their own topic");
        assert!(laid
            .taken()
            .all(|(slot, _, _)| laid.near[slot].len() == LINKS));
        assert_eq!(saved(&db), *laid);
    }
    assert!(
        !reconcile(&vectors, &map, &db, HashMap::new).unwrap(),
        "nothing to do"
    );
}

#[test]
fn a_new_note_joins_its_topic_and_only_the_notes_near_it_move() {
    let (vectors, map, db) = slots(topics(4, 12, 16));
    reconcile(&vectors, &map, &db, HashMap::new).unwrap();
    let before = places(&map);

    // One more note about topic 2.
    let mut v = vec![0.0; 16];
    v[0] = 1.0;
    v[3] = 1.0;
    vectors
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .insert("99902".into(), "h".into(), v)
        .unwrap();
    assert!(reconcile(&vectors, &map, &db, HashMap::new).unwrap());

    let after = places(&map);
    assert!(
        unmoved(&before, &after) >= 48 - 2 * LINKS,
        "only the notes around the new one move"
    );
    let map = map.lock().unwrap();
    let map = map.as_ref().unwrap();
    let new = map.get("99902").unwrap();
    let mine = distance(new, map.get("00002").unwrap());
    let other = distance(new, map.get("00000").unwrap());
    assert!(mine < other, "{mine} from its topic, {other} from another");
    let near = near_ids(map, "99902");
    assert_eq!(near.len(), LINKS);
    assert!(near.iter().take(5).all(|id| id.ends_with("02")));
    assert_eq!(saved(&db), *map);
}

#[test]
fn deleted_and_edited_notes_never_lay_the_space_out_again() {
    let (vectors, map, db) = slots(topics(4, 12, 16));
    reconcile(&vectors, &map, &db, HashMap::new).unwrap();

    // A note deleted leaves every list, which takes the next closest.
    let present = (1..48)
        .map(|i| format!("{:03}{:02}", i / 4, i % 4))
        .collect();
    vectors.lock().unwrap().as_mut().unwrap().retain(&present);
    let before = places(&map);
    assert!(reconcile(&vectors, &map, &db, HashMap::new).unwrap());
    {
        let map = map.lock().unwrap();
        let map = map.as_ref().unwrap();
        assert_eq!(map.len(), 47);
        assert!(map.taken().all(|(_, id, _)| {
            let near = near_ids(map, id);
            near.len() == LINKS && near.iter().all(|other| other != "00000")
        }));
        assert_eq!(
            unmoved(&before, &places_of(map)),
            47,
            "nothing moves for a deletion"
        );
        assert_eq!(saved(&db), *map);
    }

    // Every note written again, one pass after another: never a full
    // layout, however many.
    let ids: Vec<String> = places(&map).keys().cloned().collect();
    for id in ids {
        let before = places(&map);
        {
            let mut slot = vectors.lock().unwrap();
            let store = slot.as_mut().unwrap();
            let v = store.get(&id).unwrap().1.to_vec();
            store.insert(id.clone(), "edited".into(), v).unwrap();
        }
        assert!(reconcile(&vectors, &map, &db, HashMap::new).unwrap());
        assert!(unmoved(&before, &places(&map)) >= 47 - 2 * LINKS);
    }
    let map = map.lock().unwrap();
    let map = map.as_ref().unwrap();
    assert!(nearest_same_topic_of_47(map) >= 0.9);
    assert_eq!(saved(&db), *map);
}

/// `nearest_same_topic` for the 47 notes left once `00000` is gone.
fn nearest_same_topic_of_47(map: &Map) -> f32 {
    let notes: Vec<(&str, [f32; 2])> = map
        .taken()
        .map(|(slot, id, _)| (&id[3..], map.at[slot]))
        .collect();
    let same = notes
        .iter()
        .enumerate()
        .filter(|(i, (topic, at))| {
            let nearest = notes
                .iter()
                .enumerate()
                .filter(|(j, _)| j != i)
                .min_by(|a, b| distance(*at, a.1 .1).total_cmp(&distance(*at, b.1 .1)))
                .unwrap();
            nearest.1 .0 == *topic
        })
        .count();
    same as f32 / notes.len() as f32
}

#[test]
fn each_note_links_to_its_closest_once() {
    let (vectors, map, db) = slots(topics(4, 12, 16));
    reconcile(&vectors, &map, &db, HashMap::new).unwrap();
    let map = map.lock().unwrap();
    let map = map.as_ref().unwrap();
    let links = map.links(3);
    let pairs: HashSet<(&str, &str)> = links.iter().copied().collect();
    assert_eq!(pairs.len(), links.len(), "each pair once");
    for (slot, id, _) in map.taken() {
        for &(_, to) in &map.near[slot][..3] {
            let other = map.id(to);
            assert!(pairs.contains(&(id, other)) || pairs.contains(&(other, id)));
        }
    }
    // Two notes about one topic, closest to each other, make one link.
    assert!(links.len() < 48 * 3);
}

#[test]
fn vectors_from_another_model_lay_the_space_out_again() {
    let (vectors, map, db) = slots(topics(2, 5, 8));
    reconcile(&vectors, &map, &db, HashMap::new).unwrap();
    let mut other = Vectors::new("other", 8);
    for (id, hash, v) in vectors.lock().unwrap().as_ref().unwrap().iter() {
        other
            .insert(id.to_string(), hash.to_string(), v.to_vec())
            .unwrap();
    }
    *vectors.lock().unwrap() = Some(other);
    assert!(reconcile(&vectors, &map, &db, HashMap::new).unwrap());
    assert_eq!(map.lock().unwrap().as_ref().unwrap().model, "other");
    assert_eq!(saved(&db).model, "other");
}

#[test]
fn one_note_or_none_is_placed_without_failing() {
    let (vectors, map, db) = slots(Vectors::new("m", 4));
    assert!(!reconcile(&vectors, &map, &db, HashMap::new).unwrap());
    for id in ["01A", "01B"] {
        vectors
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .insert(id.into(), "h".into(), vec![1.0, 0.0, 0.0, 0.0])
            .unwrap();
        assert!(reconcile(&vectors, &map, &db, HashMap::new).unwrap());
    }
    assert_eq!(map.lock().unwrap().as_ref().unwrap().len(), 2);
}

/// The vectors of the space at `SCRATCHNOTE_BENCH_SPACE`, from its
/// `vectors.bin` or its `space.db`, read only.
fn bench_vectors() -> Vectors {
    let root = std::path::PathBuf::from(std::env::var("SCRATCHNOTE_BENCH_SPACE").unwrap());
    let bin = root.join(".scratchnote").join("vectors.bin");
    Vectors::read_bin(&bin).unwrap_or_else(|| {
        let db = rusqlite::Connection::open_with_flags(
            crate::storage::space_db::space_db_path(&root),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let model: String = db
            .query_row(
                "SELECT value FROM meta WHERE key = 'vectors_model'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let mut vectors = Vectors::new(&model, 768);
        let mut statement = db.prepare("SELECT id, hash, vector FROM vectors").unwrap();
        let mut rows = statement.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let bytes: Vec<u8> = row.get(2).unwrap();
            let v = crate::embed::math::from_le_bytes(&bytes);
            vectors
                .insert(row.get(0).unwrap(), row.get(1).unwrap(), v)
                .unwrap();
        }
        vectors
    })
}

/// On a real space: how long a full layout and one saved note take, and
/// how many of each note's 10 closest notes by meaning are among its 10
/// closest on the map, laid out afresh and when the newest half was added
/// one by one without ever laying the space out again. Reads the
/// space's `vectors.bin`, or its `space.db`, never writes either. In
/// release: `SCRATCHNOTE_BENCH_SPACE=<space folder> cargo test --release
/// bench_map -- --ignored --nocapture`.
#[test]
#[ignore]
fn bench_map() {
    use std::time::Instant;
    let all = bench_vectors();
    let snapshot = Snapshot::of(&all);
    let n = snapshot.len();
    let truth = closest(&snapshot, 10);
    let kept = |map: &Map, rows: std::ops::Range<usize>| {
        let y: Vec<[f32; 2]> = snapshot.ids.iter().map(|id| map.get(id).unwrap()).collect();
        let mut sum = 0.0;
        for i in rows.clone() {
            let mut near: Vec<(f32, usize)> = (0..n)
                .filter(|&j| j != i)
                .map(|j| (distance(y[i], y[j]), j))
                .collect();
            near.select_nth_unstable_by(9, |a, b| a.0.total_cmp(&b.0));
            let near: HashSet<usize> = near[..10].iter().map(|(_, j)| *j).collect();
            sum += truth[i].iter().filter(|(_, j)| near.contains(j)).count() as f64 / 10.0;
        }
        100.0 * sum / rows.len() as f64
    };

    // As the embed task does it: laid out once, then notes saved later.
    let mut copy = Vectors::new(all.model_id(), all.dims());
    for (id, hash, v) in all.iter() {
        copy.insert(id.to_string(), hash.to_string(), v.to_vec())
            .unwrap();
    }
    let (vectors, map, db) = slots(copy);
    let started = Instant::now();
    reconcile(&vectors, &map, &db, HashMap::new).unwrap();
    println!(
        "{n} notes: laid out and saved in {:?}, {:.0}% of closest notes kept",
        started.elapsed(),
        kept(map.lock().unwrap().as_ref().unwrap(), 0..n)
    );
    let one = all.get(&snapshot.ids[n / 2]).unwrap().1.to_vec();
    let mut runs = Vec::new();
    for run in 0..15 {
        let mut v = one.clone();
        v[run] += 0.05;
        vectors
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .insert(format!("ZZ{run:02}"), "h".into(), v)
            .unwrap();
        let started = Instant::now();
        reconcile(&vectors, &map, &db, HashMap::new).unwrap();
        runs.push(started.elapsed());
    }
    runs.sort();
    println!(
        "one note saved later, placed, settled and saved: {:?} (median of 15, slowest {:?})",
        runs[7], runs[14]
    );

    // Where the time of a full layout and of one saved note goes.
    let t = Instant::now();
    let mut y = principal(&snapshot);
    let t_pca = t.elapsed();
    let t = Instant::now();
    let near = closest(&snapshot, LINKS);
    let t_near = t.elapsed();
    let t = Instant::now();
    let everyone: Vec<usize> = (0..n).collect();
    let movable = vec![true; n];
    let settle = Settle::everyone(&everyone, &movable);
    pull_and_push(&mut y, &near, &settle, &mut Random::new());
    println!(
        "pca {t_pca:?}, closest {t_near:?}, pull and push {:?}",
        t.elapsed()
    );
    let started = Instant::now();
    let mut whole = Map::default();
    whole.lay_out(&snapshot);
    let laying = started.elapsed();
    let dir = std::env::temp_dir().join("scratchnote-bench-map");
    let _ = std::fs::remove_dir_all(&dir);
    let mut fresh = SpaceDb::open(&dir).unwrap();
    let before =
        std::fs::metadata(crate::storage::space_db::space_db_path(&dir)).map_or(0, |m| m.len());
    let started = Instant::now();
    fresh.transaction(|tx| whole.write_all(tx)).unwrap();
    let writing = started.elapsed();
    fresh
        .conn()
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
        .unwrap();
    let after =
        std::fs::metadata(crate::storage::space_db::space_db_path(&dir)).map_or(0, |m| m.len());
    println!(
        "full layout {laying:?}, writing it {writing:?}, {:.1} MB on disk",
        (after - before) as f64 / 1e6
    );
    drop(fresh);
    let _ = std::fs::remove_dir_all(&dir);
    {
        let held = vectors.lock().unwrap();
        let mut map = map.lock().unwrap();
        let map = map.as_mut().unwrap();
        let mut v = one.clone();
        v[20] += 0.05;
        drop(held);
        vectors
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .insert("ZZ99".into(), "h".into(), v)
            .unwrap();
        let held = vectors.lock().unwrap();
        let store = held.as_ref().unwrap();
        let t = Instant::now();
        let behind = map.behind(store);
        let t_behind = t.elapsed();
        let t = Instant::now();
        let snap = Snapshot::of(store);
        let t_snap = t.elapsed();
        let t = Instant::now();
        let changes = map.update(&snap);
        let t_update = t.elapsed();
        let t = Instant::now();
        db.lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .transaction(|tx| map.write_changes(tx, &changes))
            .unwrap();
        println!(
            "one note: behind {behind} {t_behind:?}, copy {t_snap:?}, update {t_update:?}, write {:?} ({} places, {} lists)",
            t.elapsed(),
            changes.moved.len(),
            changes.lists.len()
        );
        let _ = store;
    }

    // Never laid out again: the oldest half laid out, then the newest
    // half added one by one, against the same mean as the whole.
    let cut = n / 2;
    let older = Snapshot {
        model: snapshot.model.clone(),
        ids: snapshot.ids[..cut].to_vec(),
        hashes: snapshot.hashes[..cut].to_vec(),
        dims: snapshot.dims,
        rows: snapshot.rows[..cut * snapshot.dims].to_vec(),
    };
    let mut grown = Map::default();
    grown.lay_out(&older);
    let first = places_of(&grown);
    let started = Instant::now();
    grown.update(&snapshot);
    let each = started.elapsed() / (n - cut) as u32;
    let centre = first.values().fold([0.0, 0.0], |c, (_, p)| {
        [c[0] + p[0] / cut as f32, c[1] + p[1] / cut as f32]
    });
    let radius = (first
        .values()
        .map(|(_, p)| distance(*p, centre).powi(2))
        .sum::<f32>()
        / cut as f32)
        .sqrt();
    let moved = first
        .iter()
        .map(|(id, (_, p))| distance(*p, grown.get(id).unwrap()))
        .sum::<f32>()
        / cut as f32;
    println!(
        "newest {} added one by one: {each:?} each, {:.0}% kept overall, {:.0}% for them, older notes moved {:.1}% of the map's radius on average",
        n - cut,
        kept(&grown, 0..n),
        kept(&grown, cut..n),
        100.0 * moved / radius
    );
}
