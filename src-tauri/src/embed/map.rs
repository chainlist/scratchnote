//! Where each note sits on a 2D map of the space by meaning, saved in the
//! `positions` table of `space.db` so that no view lays the space out
//! itself.
//!
//! A full layout places every note: each is pulled towards its closest notes
//! and pushed away from others, as UMAP does, starting from the two
//! directions the notes differ most along. It compares every note with every
//! other, seconds for 10,000 notes, so it runs in the background, and only
//! when a space has no places yet, they came from another model's vectors,
//! or over a tenth of its notes were placed one by one since. Between full
//! layouts, a note new or written again goes to the average of its closest
//! notes' places, weighted by how close, and no other note moves.
//!
//! Closeness is the cosine once the mean of every note is taken off, as for
//! Similar notes: notes share a lot just by being short notes by one person.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use rusqlite::Connection;

use super::vectors::{dot, Vectors};
use crate::storage::space_db::{to_string, SpaceDb};

/// The closest notes each note is pulled towards in a full layout.
const LINKS: usize = 15;
/// The closest placed notes a note is put among when placed on its own.
const PLACE_AMONG: usize = 5;
/// Passes over every link. UMAP's own default for a space this size.
const EPOCHS: usize = 300;
/// Notes a note is pushed away from for each link it is pulled along.
const PUSHES: usize = 5;
/// The share of notes placed one by one after which the space is laid out
/// again whole.
const DRIFT: f64 = 0.1;

#[derive(Debug, Default, PartialEq)]
pub struct Map {
    /// The model of the vectors the places came from.
    model: String,
    /// Note id to the body hash it was placed from and its place.
    places: HashMap<String, (String, [f32; 2])>,
    /// Notes placed one by one since the last full layout.
    since: usize,
}

/// What a layout reads from the vectors, copied so that they are let go
/// while it runs: ids in order, and every vector with the mean taken off,
/// at unit length.
struct Snapshot {
    model: String,
    ids: Vec<String>,
    hashes: Vec<String>,
    dims: usize,
    rows: Vec<f32>,
}

impl Snapshot {
    fn of(vectors: &Vectors) -> Self {
        let n = vectors.len();
        let dims = vectors.dims();
        let mean: Vec<f32> = vectors.sum().iter().map(|s| s / n.max(1) as f32).collect();
        let mut notes: Vec<(&str, &str, &[f32])> = vectors.iter().collect();
        notes.sort_by(|a, b| a.0.cmp(b.0));
        let mut rows = Vec::with_capacity(n * dims);
        for (_, _, vector) in &notes {
            let start = rows.len();
            rows.extend(vector.iter().zip(&mean).map(|(x, m)| x - m));
            let norm = dot(&rows[start..], &rows[start..]).sqrt();
            if norm > 0.0 {
                rows[start..].iter_mut().for_each(|x| *x /= norm);
            }
        }
        Self {
            model: vectors.model_id().to_string(),
            ids: notes.iter().map(|(id, _, _)| id.to_string()).collect(),
            hashes: notes.iter().map(|(_, hash, _)| hash.to_string()).collect(),
            dims,
            rows,
        }
    }

    fn len(&self) -> usize {
        self.ids.len()
    }

    fn row(&self, i: usize) -> &[f32] {
        &self.rows[i * self.dims..(i + 1) * self.dims]
    }
}

/// What a pass has to do.
enum Work {
    /// Lay out every note.
    Full,
    /// Forget `gone` and place `todo`, by index into the snapshot, one by
    /// one among `known`.
    Place {
        gone: Vec<String>,
        todo: Vec<usize>,
        known: HashMap<String, [f32; 2]>,
    },
}

impl Map {
    /// The saved places. Never an error: they can always be laid out again.
    pub fn load(db: &SpaceDb) -> Self {
        let Some(model) = db.meta("map_model") else {
            return Self::default();
        };
        let since = db
            .meta("map_since")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        match Self::read(db.conn()) {
            Ok(places) => Self {
                model,
                places,
                since,
            },
            Err(e) => {
                log::warn!("could not read the map: {e}");
                Self::default()
            }
        }
    }

    fn read(conn: &Connection) -> rusqlite::Result<HashMap<String, (String, [f32; 2])>> {
        let mut statement = conn.prepare("SELECT id, hash, x, y FROM positions")?;
        let rows = statement
            .query_map([], |row| {
                let place = [row.get::<_, f64>(2)? as f32, row.get::<_, f64>(3)? as f32];
                Ok((row.get(0)?, (row.get(1)?, place)))
            })?
            .collect();
        rows
    }

    /// Where a note is, `None` until it is placed.
    pub fn get(&self, id: &str) -> Option<[f32; 2]> {
        self.places.get(id).map(|(_, place)| *place)
    }

    pub fn len(&self) -> usize {
        self.places.len()
    }

    /// What a pass has to do for these vectors, `None` when nothing.
    fn work(
        &self,
        vectors: &Vectors,
        snapshot: impl FnOnce() -> Snapshot,
    ) -> Option<(Work, Snapshot)> {
        let gone: Vec<String> = self
            .places
            .keys()
            .filter(|id| vectors.get(id).is_none())
            .cloned()
            .collect();
        let stale = vectors
            .iter()
            .filter(|(id, hash, _)| self.places.get(*id).is_none_or(|(h, _)| h != hash))
            .count();
        let empty = vectors.len() == 0 && self.places.is_empty();
        if empty || (gone.is_empty() && stale == 0 && self.model == vectors.model_id()) {
            return None;
        }
        let snapshot = snapshot();
        let full = self.model != vectors.model_id()
            || self.places.len() == gone.len()
            || (self.since + stale) as f64 > DRIFT * vectors.len() as f64;
        if full {
            return Some((Work::Full, snapshot));
        }
        let todo: Vec<usize> = (0..snapshot.len())
            .filter(|&i| {
                let id = &snapshot.ids[i];
                self.places
                    .get(id)
                    .is_none_or(|(h, _)| *h != snapshot.hashes[i])
            })
            .collect();
        let redo: HashSet<&str> = todo.iter().map(|&i| snapshot.ids[i].as_str()).collect();
        let known = self
            .places
            .iter()
            .filter(|(id, _)| vectors.get(id).is_some() && !redo.contains(id.as_str()))
            .map(|(id, (_, place))| (id.clone(), *place))
            .collect();
        Some((Work::Place { gone, todo, known }, snapshot))
    }

    /// Write the whole map in place of what is saved.
    fn write_all(&self, tx: &Connection) -> Result<(), String> {
        tx.execute("DELETE FROM positions", []).map_err(to_string)?;
        SpaceDb::set_meta(tx, "map_model", &self.model)?;
        SpaceDb::set_meta(tx, "map_since", &self.since.to_string())?;
        self.write_rows(tx, self.places.keys())
    }

    fn write_rows<'a>(
        &self,
        tx: &Connection,
        ids: impl IntoIterator<Item = &'a String>,
    ) -> Result<(), String> {
        let mut put = tx
            .prepare_cached(
                "INSERT OR REPLACE INTO positions (id, hash, x, y) VALUES (?1, ?2, ?3, ?4)",
            )
            .map_err(to_string)?;
        for id in ids {
            let (hash, [x, y]) = &self.places[id];
            put.execute(rusqlite::params![id, hash, *x as f64, *y as f64])
                .map_err(to_string)?;
        }
        Ok(())
    }
}

/// Bring the saved map in line with the vectors, laying the whole space out
/// again when it must. The vectors are held only while copied, so search
/// goes on during a layout. The map is loaded from `db` first if it is not
/// in memory yet. True when any note moved.
pub fn reconcile(
    vectors: &Mutex<Option<Vectors>>,
    map: &Mutex<Option<Map>>,
    db: &Mutex<Option<SpaceDb>>,
) -> Result<bool, String> {
    let (work, snapshot) = {
        let vectors = vectors.lock().map_err(|_| "vectors lock poisoned")?;
        let Some(vectors) = vectors.as_ref() else {
            return Ok(false);
        };
        let mut slot = map.lock().map_err(|_| "map lock poisoned")?;
        if slot.is_none() {
            let db = db.lock().map_err(|_| "space.db lock poisoned")?;
            let Some(db) = db.as_ref() else {
                return Ok(false);
            };
            *slot = Some(Map::load(db));
        }
        let current = slot.as_ref().expect("loaded just above");
        match current.work(vectors, || Snapshot::of(vectors)) {
            Some(work) => work,
            None => return Ok(false),
        }
    };

    let started = std::time::Instant::now();
    let (places, full, gone, placed) = match work {
        Work::Full => (
            layout(&snapshot),
            true,
            Vec::new(),
            (0..snapshot.len()).collect(),
        ),
        Work::Place { gone, todo, known } => {
            let places = place(&snapshot, &todo, known);
            (places, false, gone, todo)
        }
    };

    let mut slot = map.lock().map_err(|_| "map lock poisoned")?;
    let mut db = db.lock().map_err(|_| "space.db lock poisoned")?;
    // The space was closed meanwhile.
    let (Some(map), Some(db)) = (slot.as_mut(), db.as_mut()) else {
        return Ok(false);
    };
    for &i in &placed {
        let id = &snapshot.ids[i];
        map.places
            .insert(id.clone(), (snapshot.hashes[i].clone(), places[id]));
    }
    if full {
        map.places.retain(|id, _| places.contains_key(id));
        map.model = snapshot.model.clone();
        map.since = 0;
        db.transaction(|tx| map.write_all(tx))?;
        log::info!(
            "laid out {} notes in {:?}",
            snapshot.len(),
            started.elapsed()
        );
    } else {
        for id in &gone {
            map.places.remove(id);
        }
        map.since += placed.len();
        let changed: Vec<String> = placed.iter().map(|&i| snapshot.ids[i].clone()).collect();
        db.transaction(|tx| {
            for id in &gone {
                tx.execute("DELETE FROM positions WHERE id = ?1", [id])
                    .map_err(to_string)?;
            }
            SpaceDb::set_meta(tx, "map_since", &map.since.to_string())?;
            map.write_rows(tx, &changed)
        })?;
    }
    Ok(true)
}

/// Place each of `todo`, in turn, at the average of the places of its
/// closest notes among those placed, weighted by how close. Each one placed
/// counts as placed for the next.
fn place(
    snapshot: &Snapshot,
    todo: &[usize],
    mut known: HashMap<String, [f32; 2]>,
) -> HashMap<String, [f32; 2]> {
    let mut placed: Vec<usize> = (0..snapshot.len())
        .filter(|&i| known.contains_key(&snapshot.ids[i]))
        .collect();
    for &i in todo {
        let mut closest: Vec<(f32, usize)> = placed
            .iter()
            .map(|&j| (dot(snapshot.row(i), snapshot.row(j)), j))
            .collect();
        let k = PLACE_AMONG.min(closest.len());
        let spot = if k == 0 {
            [0.0, 0.0]
        } else {
            closest.select_nth_unstable_by(k - 1, |a, b| b.0.total_cmp(&a.0));
            closest.truncate(k);
            let total: f32 = closest.iter().map(|(s, _)| s.max(0.0)).sum();
            let weight = |s: f32| {
                if total > 0.0 {
                    s.max(0.0) / total
                } else {
                    1.0 / k as f32
                }
            };
            closest.iter().fold([0.0, 0.0], |[x, y], &(s, j)| {
                let at = known[&snapshot.ids[j]];
                [x + weight(s) * at[0], y + weight(s) * at[1]]
            })
        };
        known.insert(snapshot.ids[i].clone(), spot);
        placed.push(i);
    }
    known
}

/// Every note's place in a full layout.
fn layout(snapshot: &Snapshot) -> HashMap<String, [f32; 2]> {
    let n = snapshot.len();
    let mut y = principal(snapshot);
    if n > 1 {
        let links = closest(snapshot, LINKS.min(n - 1));
        settle(&mut y, &links);
    }
    snapshot.ids.iter().cloned().zip(y).collect()
}

/// The `k` closest notes of every note, by dot product of the centred unit
/// rows. Every note against every other, so split over the cores.
fn closest(snapshot: &Snapshot, k: usize) -> Vec<Vec<usize>> {
    let n = snapshot.len();
    let threads = std::thread::available_parallelism().map_or(1, |c| c.get());
    let chunk = n.div_ceil(threads).max(1);
    let mut out = vec![Vec::new(); n];
    std::thread::scope(|scope| {
        for (c, slice) in out.chunks_mut(chunk).enumerate() {
            scope.spawn(move || {
                for (offset, slot) in slice.iter_mut().enumerate() {
                    let i = c * chunk + offset;
                    let mut best: Vec<(f32, usize)> = Vec::with_capacity(k + 1);
                    for j in (0..n).filter(|&j| j != i) {
                        let s = dot(snapshot.row(i), snapshot.row(j));
                        if best.len() < k || s > best[best.len() - 1].0 {
                            let at = best.partition_point(|(b, _)| *b >= s);
                            best.insert(at, (s, j));
                            best.truncate(k);
                        }
                    }
                    *slot = best.into_iter().map(|(_, j)| j).collect();
                }
            });
        }
    });
    out
}

/// The two directions the notes differ most along, by power iteration, and
/// every note's place along them, spread to a standard deviation of 10.
fn principal(snapshot: &Snapshot) -> Vec<[f32; 2]> {
    let (n, d) = (snapshot.len(), snapshot.dims);
    if n == 0 {
        return Vec::new();
    }
    let mut mean = vec![0.0f32; d];
    for i in 0..n {
        mean.iter_mut()
            .zip(snapshot.row(i))
            .for_each(|(m, x)| *m += x / n as f32);
    }
    let centred = |i: usize, w: &[f32]| dot(snapshot.row(i), w) - dot(&mean, w);
    let mut axes: Vec<Vec<f32>> = Vec::new();
    for axis in 0..2 {
        // A fixed start, so a space lays out the same way every time.
        let mut w: Vec<f32> = (0..d)
            .map(|j| 1.0 + ((j * 7 + axis * 3) % 11) as f32 / 11.0)
            .collect();
        for _ in 0..50 {
            let mut next = vec![0.0f32; d];
            for i in 0..n {
                let s = centred(i, &w);
                next.iter_mut()
                    .zip(snapshot.row(i).iter().zip(&mean))
                    .for_each(|(x, (r, m))| *x += s * (r - m));
            }
            for done in &axes {
                let along = dot(&next, done);
                next.iter_mut().zip(done).for_each(|(x, a)| *x -= along * a);
            }
            let norm = dot(&next, &next).sqrt();
            if norm == 0.0 {
                break;
            }
            next.iter_mut().for_each(|x| *x /= norm);
            w = next;
        }
        axes.push(w);
    }
    let mut y: Vec<[f32; 2]> = (0..n)
        .map(|i| [centred(i, &axes[0]), centred(i, &axes[1])])
        .collect();
    for c in 0..2 {
        let spread = (y.iter().map(|p| p[c] * p[c]).sum::<f32>() / n as f32).sqrt();
        if spread > 0.0 {
            y.iter_mut().for_each(|p| p[c] *= 10.0 / spread);
        }
    }
    y
}

/// Pull every note towards its closest notes and push it away from random
/// others, in passes of shrinking steps, with UMAP's gradients for a = b = 1.
/// A fixed random sequence, so a space lays out the same way every time.
fn settle(y: &mut [[f32; 2]], links: &[Vec<usize>]) {
    let n = y.len();
    let clip = |g: f32| g.clamp(-4.0, 4.0);
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut random = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % n as u64) as usize
    };
    for epoch in 0..EPOCHS {
        let step = 1.0 - epoch as f32 / EPOCHS as f32;
        for (i, closest) in links.iter().enumerate() {
            for &j in closest {
                let d = [y[i][0] - y[j][0], y[i][1] - y[j][1]];
                let q = -2.0 / (1.0 + d[0] * d[0] + d[1] * d[1]);
                for c in 0..2 {
                    let g = step * clip(q * d[c]);
                    y[i][c] += g;
                    y[j][c] -= g;
                }
                for _ in 0..PUSHES {
                    let r = random();
                    if r == i {
                        continue;
                    }
                    let d = [y[i][0] - y[r][0], y[i][1] - y[r][1]];
                    let dd = d[0] * d[0] + d[1] * d[1];
                    let q = 2.0 / ((0.001 + dd) * (1.0 + dd));
                    for c in 0..2 {
                        y[i][c] += step * clip(q * d[c]);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn slots(
        vectors: Vectors,
    ) -> (
        Mutex<Option<Vectors>>,
        Mutex<Option<Map>>,
        Mutex<Option<SpaceDb>>,
    ) {
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

    #[test]
    fn a_layout_gathers_each_topic_apart_from_the_others_and_is_saved() {
        let (vectors, map, db) = slots(topics(4, 12, 16));
        assert!(reconcile(&vectors, &map, &db).unwrap());
        {
            let laid = map.lock().unwrap();
            let laid = laid.as_ref().unwrap();
            assert_eq!(laid.len(), 48);
            let same = nearest_same_topic(laid, 4, 12);
            assert!(same >= 0.95, "{same} of notes are nearest their own topic");
            assert_eq!(Map::load(db.lock().unwrap().as_ref().unwrap()), *laid);
        }
        assert!(!reconcile(&vectors, &map, &db).unwrap(), "nothing to do");
    }

    #[test]
    fn a_new_note_joins_its_topic_and_nothing_else_moves() {
        let (vectors, map, db) = slots(topics(4, 12, 16));
        reconcile(&vectors, &map, &db).unwrap();
        let before = map.lock().unwrap().as_ref().unwrap().places.clone();

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
        assert!(reconcile(&vectors, &map, &db).unwrap());

        let map = map.lock().unwrap();
        let map = map.as_ref().unwrap();
        assert_eq!(map.since, 1);
        for (id, place) in &before {
            assert_eq!(map.places[id], *place, "{id} moved");
        }
        let new = map.get("99902").unwrap();
        let mine = distance(new, map.get("00002").unwrap());
        let other = distance(new, map.get("00000").unwrap());
        assert!(mine < other, "{mine} from its topic, {other} from another");
        assert_eq!(Map::load(db.lock().unwrap().as_ref().unwrap()), *map);
    }

    #[test]
    fn a_deleted_note_leaves_and_drift_lays_the_space_out_again() {
        let (vectors, map, db) = slots(topics(4, 12, 16));
        reconcile(&vectors, &map, &db).unwrap();

        let present = (1..48)
            .map(|i| format!("{:03}{:02}", i / 4, i % 4))
            .collect();
        vectors.lock().unwrap().as_mut().unwrap().retain(&present);
        assert!(reconcile(&vectors, &map, &db).unwrap());
        assert_eq!(map.lock().unwrap().as_ref().unwrap().len(), 47);

        // Edit more than a tenth of the notes: placing them one by one would
        // drift, so the whole space is laid out again.
        {
            let mut slot = vectors.lock().unwrap();
            let store = slot.as_mut().unwrap();
            let edited: Vec<(String, Vec<f32>)> = store
                .iter()
                .take(6)
                .map(|(id, _, v)| (id.to_string(), v.to_vec()))
                .collect();
            for (id, v) in edited {
                store.insert(id, "edited".into(), v).unwrap();
            }
        }
        assert!(reconcile(&vectors, &map, &db).unwrap());
        let map = map.lock().unwrap();
        assert_eq!(map.as_ref().unwrap().since, 0);
        assert_eq!(map.as_ref().unwrap().len(), 47);
    }

    #[test]
    fn vectors_from_another_model_lay_the_space_out_again() {
        let (vectors, map, db) = slots(topics(2, 5, 8));
        reconcile(&vectors, &map, &db).unwrap();
        let mut other = Vectors::new("other", 8);
        for (id, hash, v) in vectors.lock().unwrap().as_ref().unwrap().iter() {
            other
                .insert(id.to_string(), hash.to_string(), v.to_vec())
                .unwrap();
        }
        *vectors.lock().unwrap() = Some(other);
        assert!(reconcile(&vectors, &map, &db).unwrap());
        assert_eq!(map.lock().unwrap().as_ref().unwrap().model, "other");
    }

    /// How long a full layout and placing one note take on a real space,
    /// and how many of each note's 10 closest notes by meaning are among its
    /// 10 closest on the map. The oldest 80% are laid out, then the newest
    /// placed one by one, as if each was saved later. Reads the space's
    /// `vectors.bin`, or its `space.db`, never writes either. In release:
    /// `SCRATCHNOTE_BENCH_SPACE=<space folder> cargo test --release
    /// bench_map -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bench_map() {
        use std::time::Instant;
        let root = std::path::PathBuf::from(std::env::var("SCRATCHNOTE_BENCH_SPACE").unwrap());
        let bin = root.join(".scratchnote").join("vectors.bin");
        let all = Vectors::read_bin(&bin).unwrap_or_else(|| {
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
                let v = bytes
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .collect();
                vectors
                    .insert(row.get(0).unwrap(), row.get(1).unwrap(), v)
                    .unwrap();
            }
            vectors
        });
        let started = Instant::now();
        let snapshot = Snapshot::of(&all);
        let n = snapshot.len();
        println!("{n} notes, copied for a layout in {:?}", started.elapsed());

        // As the embed task does it: the whole space laid out and saved,
        // then one note saved later.
        let mut copy = Vectors::new(all.model_id(), all.dims());
        for (id, hash, v) in all.iter() {
            copy.insert(id.to_string(), hash.to_string(), v.to_vec())
                .unwrap();
        }
        let (vectors, map, db) = slots(copy);
        let started = Instant::now();
        reconcile(&vectors, &map, &db).unwrap();
        println!("first layout, saved: {:?}", started.elapsed());
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
            reconcile(&vectors, &map, &db).unwrap();
            runs.push(started.elapsed());
        }
        runs.sort();
        println!(
            "one note saved later, placed and saved: {:?} (median of 15)",
            runs[7]
        );

        let started = Instant::now();
        let truth = closest(&snapshot, 10);
        println!("10 closest of every note: {:?}", started.elapsed());

        let kept = |places: &HashMap<String, [f32; 2]>, rows: &mut dyn Iterator<Item = usize>| {
            let at: Vec<[f32; 2]> = snapshot.ids.iter().map(|id| places[id]).collect();
            let (mut sum, mut count) = (0.0, 0);
            for i in rows {
                let mut near: Vec<(f32, usize)> = (0..n)
                    .filter(|&j| j != i)
                    .map(|j| (distance(at[i], at[j]), j))
                    .collect();
                near.select_nth_unstable_by(9, |a, b| a.0.total_cmp(&b.0));
                let near: HashSet<usize> = near[..10].iter().map(|(_, j)| *j).collect();
                sum += truth[i].iter().filter(|j| near.contains(j)).count() as f64 / 10.0;
                count += 1;
            }
            sum / count as f64
        };

        let started = Instant::now();
        let whole = layout(&snapshot);
        let took = started.elapsed();
        println!(
            "full layout: {took:?}, {:.0}% of closest notes kept",
            100.0 * kept(&whole, &mut (0..n))
        );

        // The oldest 80%, laid out alone, then the rest placed one by one.
        let cut = n * 8 / 10;
        let mut older = Vectors::new(all.model_id(), all.dims());
        for id in &snapshot.ids[..cut] {
            let (hash, v) = all.get(id).unwrap();
            older
                .insert(id.clone(), hash.to_string(), v.to_vec())
                .unwrap();
        }
        let older = Snapshot::of(&older);
        let base = layout(&older);
        let started = Instant::now();
        let placed = place(&snapshot, &(cut..n).collect::<Vec<_>>(), base);
        let each = started.elapsed() / (n - cut) as u32;
        println!(
            "placing the newest {} one by one: {each:?} each, {:.0}% kept overall, {:.0}% for them",
            n - cut,
            100.0 * kept(&placed, &mut (0..n)),
            100.0 * kept(&placed, &mut (cut..n))
        );
    }

    #[test]
    fn one_note_or_none_is_placed_without_failing() {
        let (vectors, map, db) = slots(Vectors::new("m", 4));
        assert!(!reconcile(&vectors, &map, &db).unwrap());
        vectors
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .insert("01A".into(), "h".into(), vec![1.0, 0.0, 0.0, 0.0])
            .unwrap();
        assert!(reconcile(&vectors, &map, &db).unwrap());
        assert_eq!(map.lock().unwrap().as_ref().unwrap().len(), 1);
    }
}
