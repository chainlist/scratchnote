//! Where each note sits on a 2D map of the space by meaning, saved in the
//! `positions` table of `space.db` so that no view lays the space out
//! itself, with each note's closest notes in `links`.
//!
//! The map is laid out whole only when a space has none yet, or its notes
//! were embedded by another model: each note is pulled towards its closest
//! notes and pushed away from others, as UMAP does, starting from the two
//! directions the notes differ most along. That compares every note with
//! every other, seconds for 10,000 notes, so it runs in the background.
//!
//! After that it is never laid out again. A note new or written again gets
//! its closest notes, joins the lists of the notes it is closer to than
//! their own last, starts at the average of its closest notes' places,
//! weighted by how close, and then it and the notes around it settle for a
//! few rounds of the same pulls and pushes. A deleted note leaves every
//! list it was in, and those lists take the next closest note. Measured on
//! 10,269 notes, adding the newest half that way keeps nearly as many of
//! each note's closest notes nearby as laying them all out afresh, at about
//! 2 ms a note.
//!
//! Closeness is the cosine once the mean of every note is taken off, as for
//! Similar notes: notes share a lot just by being short notes by one person.
//! A note's list keeps the scores it was made with as the mean moves.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Mutex;

use rusqlite::Connection;

use super::vectors::{dot, Vectors};
use crate::storage::space_db::{to_string, SpaceDb};

/// The closest notes each note is pulled towards.
const LINKS: usize = 15;
/// The closest notes a note new or written again starts among.
const PLACE_AMONG: usize = 5;
/// Rounds of a full layout. UMAP's own default for a space this size.
const EPOCHS: usize = 300;
/// Notes a note is pushed away from for each note it is pulled towards.
const PUSHES: usize = 5;
/// Rounds a note new or written again settles for with the notes around
/// it, and the step they start at: UMAP's own for points added to a map.
const SETTLE_EPOCHS: usize = 30;
const SETTLE_STEP: f32 = 0.25;

/// The map, by slot: a note keeps its slot while it is on the map, so the
/// lists of closest notes hold slots, and placing a note looks nothing up
/// by id. Ids are only for loading and saving.
#[derive(Debug, Default)]
pub struct Map {
    /// The model of the vectors the places came from.
    model: String,
    /// Note id to its slot.
    slots: HashMap<String, usize>,
    /// By slot: the note's id and the body hash it was placed from, `None`
    /// for a slot free since its note went.
    notes: Vec<Option<(String, String)>>,
    /// By slot: the note's place.
    at: Vec<[f32; 2]>,
    /// By slot: its closest notes and how close, closest first.
    near: Vec<Near>,
    /// Slots free for the next notes.
    free: Vec<usize>,
}

/// The same notes in the same places with the same closest notes, whatever
/// their slots.
impl PartialEq for Map {
    fn eq(&self, other: &Self) -> bool {
        self.model == other.model && self.by_id() == other.by_id()
    }
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

    fn score(&self, i: usize, j: usize) -> f32 {
        dot(self.row(i), self.row(j))
    }
}

/// A note as `Map::by_id` gives it: its body hash, place and closest
/// notes by id.
type Entry<'a> = (&'a str, [f32; 2], Vec<(&'a str, f32)>);

/// A list of closest notes and how close, closest first.
type Near = Vec<(f32, usize)>;

/// What an update changed, to be saved, by note id.
struct Changes {
    moved: Vec<String>,
    gone: Vec<String>,
    lists: Vec<String>,
}

impl Map {
    /// The saved map. Never an error: it can always be laid out again.
    pub fn load(db: &SpaceDb) -> Self {
        let Some(model) = db.meta("map_model") else {
            return Self::default();
        };
        Self::read(db.conn(), model).unwrap_or_else(|e| {
            log::warn!("could not read the map: {e}");
            Self::default()
        })
    }

    fn read(conn: &Connection, model: String) -> rusqlite::Result<Self> {
        let mut map = Self {
            model,
            ..Self::default()
        };
        let mut statement = conn.prepare("SELECT id, hash, x, y FROM positions ORDER BY id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            map.slots.insert(id.clone(), map.notes.len());
            map.notes.push(Some((id, row.get(1)?)));
            map.at
                .push([row.get::<_, f64>(2)? as f32, row.get::<_, f64>(3)? as f32]);
            map.near.push(Vec::new());
        }
        let mut statement = conn.prepare("SELECT id, other, score FROM links")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let (id, other): (String, String) = (row.get(0)?, row.get(1)?);
            if let (Some(&slot), Some(&to)) = (map.slots.get(&id), map.slots.get(&other)) {
                map.near[slot].push((row.get::<_, f64>(2)? as f32, to));
            }
        }
        for near in &mut map.near {
            near.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        }
        Ok(map)
    }

    /// Where a note is, `None` until it is placed. Nothing shows the map
    /// yet, so only tests read it.
    #[cfg(test)]
    pub fn get(&self, id: &str) -> Option<[f32; 2]> {
        self.slots.get(id).map(|&slot| self.at[slot])
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Every note by id: its body hash, place and closest notes.
    fn by_id(&self) -> BTreeMap<&str, Entry<'_>> {
        self.taken()
            .map(|(slot, id, hash)| {
                let near = self.near[slot]
                    .iter()
                    .map(|&(s, to)| (self.id(to), s))
                    .collect();
                (id, (hash, self.at[slot], near))
            })
            .collect()
    }

    /// Every slot in use, with its note's id and body hash.
    fn taken(&self) -> impl Iterator<Item = (usize, &str, &str)> {
        self.notes.iter().enumerate().filter_map(|(slot, note)| {
            note.as_ref()
                .map(|(id, hash)| (slot, id.as_str(), hash.as_str()))
        })
    }

    fn id(&self, slot: usize) -> &str {
        self.notes[slot].as_ref().map_or("", |(id, _)| id)
    }

    /// Whether the vectors hold a note this map does not have as it is, or
    /// the map a note they no longer hold.
    fn behind(&self, vectors: &Vectors) -> bool {
        let empty = vectors.len() == 0 && self.slots.is_empty();
        (self.model != vectors.model_id() && !empty)
            || self.taken().any(|(_, id, _)| vectors.get(id).is_none())
            || vectors.iter().any(|(id, hash, _)| {
                self.slots
                    .get(id)
                    .is_none_or(|&slot| self.notes[slot].as_ref().is_none_or(|(_, h)| h != hash))
            })
    }

    /// Lay every note out afresh. Its slots are the snapshot's order.
    fn lay_out(&mut self, snapshot: &Snapshot) {
        let (at, near) = layout(snapshot);
        *self = Self {
            model: snapshot.model.clone(),
            slots: (0..snapshot.len())
                .map(|i| (snapshot.ids[i].clone(), i))
                .collect(),
            notes: (0..snapshot.len())
                .map(|i| Some((snapshot.ids[i].clone(), snapshot.hashes[i].clone())))
                .collect(),
            at,
            near,
            free: Vec::new(),
        };
    }

    /// Forget the notes gone, and place the notes new or written again one
    /// by one, each settling with the notes around it.
    fn update(&mut self, snapshot: &Snapshot) -> Changes {
        // Which row of the snapshot each slot's note is.
        let mut row = vec![usize::MAX; self.notes.len()];
        let mut placed_row = vec![false; snapshot.len()];
        for (i, id) in snapshot.ids.iter().enumerate() {
            if let Some(&slot) = self.slots.get(id) {
                row[slot] = i;
                placed_row[i] = true;
            }
        }

        // Notes gone, and notes written again, leave the map and every list.
        let mut out = vec![false; self.notes.len()];
        let mut gone = Vec::new();
        for slot in 0..self.notes.len() {
            let Some((id, hash)) = &self.notes[slot] else {
                continue;
            };
            if row[slot] == usize::MAX {
                gone.push(id.clone());
                out[slot] = true;
            } else if *hash != snapshot.hashes[row[slot]] {
                placed_row[row[slot]] = false;
                out[slot] = true;
            }
        }
        for slot in (0..self.notes.len()).filter(|&slot| out[slot]) {
            if let Some((id, _)) = self.notes[slot].take() {
                self.slots.remove(&id);
            }
            self.near[slot].clear();
            self.free.push(slot);
        }
        let mut lists: HashSet<usize> = HashSet::new();
        let mut short = Vec::new();
        let mut pool = Vec::with_capacity(snapshot.len());
        for slot in 0..self.notes.len() {
            if self.notes[slot].is_none() {
                continue;
            }
            pool.push(slot);
            let before = self.near[slot].len();
            self.near[slot].retain(|&(_, to)| !out[to]);
            if self.near[slot].len() < before {
                short.push(slot);
            }
        }

        // A list that lost a note takes the next closest.
        for &slot in &short {
            self.near[slot] = top(slot, &pool, |a, b| snapshot.score(row[a], row[b]));
            lists.insert(slot);
        }

        let mut moved: HashSet<usize> = HashSet::new();
        let mut random = Random::new();
        for i in (0..snapshot.len()).filter(|&i| !placed_row[i]) {
            let slot = self.free.pop().unwrap_or_else(|| {
                self.notes.push(None);
                self.at.push([0.0, 0.0]);
                self.near.push(Vec::new());
                row.push(usize::MAX);
                self.notes.len() - 1
            });
            row[slot] = i;
            self.notes[slot] = Some((snapshot.ids[i].clone(), snapshot.hashes[i].clone()));
            self.slots.insert(snapshot.ids[i].clone(), slot);

            // It joins the lists of the notes it is closer to than their
            // last, and gets its own.
            let scored: Near = pool
                .iter()
                .map(|&j| (snapshot.score(i, row[j]), j))
                .collect();
            let mut touched = Vec::new();
            for &(s, j) in &scored {
                let list = &mut self.near[j];
                if list.len() < LINKS || s > list[list.len() - 1].0 {
                    let at = list.partition_point(|(b, _)| *b >= s);
                    list.insert(at, (s, slot));
                    list.truncate(LINKS);
                    touched.push(j);
                    lists.insert(j);
                }
            }
            self.near[slot] = best(scored);
            lists.insert(slot);
            self.at[slot] = average(&self.near[slot], &self.at);
            pool.push(slot);

            let mut moving: Vec<usize> = self.near[slot].iter().map(|(_, j)| *j).collect();
            moving.push(slot);
            moving.extend(touched);
            moving.sort_unstable();
            moving.dedup();
            let mut movable = vec![false; self.notes.len()];
            moving.iter().for_each(|&m| movable[m] = true);
            pull_and_push(
                &mut self.at,
                &self.near,
                &moving,
                &movable,
                &pool,
                SETTLE_EPOCHS,
                SETTLE_STEP,
                &mut random,
            );
            moved.extend(moving);
        }

        let ids =
            |slots: HashSet<usize>| slots.into_iter().map(|s| self.id(s).to_string()).collect();
        Changes {
            moved: ids(moved),
            gone,
            lists: ids(lists),
        }
    }

    /// Write the whole map in place of what is saved.
    fn write_all(&self, tx: &Connection) -> Result<(), String> {
        tx.execute_batch("DELETE FROM positions; DELETE FROM links;")
            .map_err(to_string)?;
        SpaceDb::set_meta(tx, "map_model", &self.model)?;
        let slots: Vec<usize> = self.taken().map(|(slot, _, _)| slot).collect();
        self.write_places(tx, &slots)?;
        self.write_lists(tx, &slots)
    }

    /// Write what an update changed.
    fn write_changes(&self, tx: &Connection, changes: &Changes) -> Result<(), String> {
        for id in &changes.gone {
            tx.execute("DELETE FROM positions WHERE id = ?1", [id])
                .map_err(to_string)?;
            tx.execute("DELETE FROM links WHERE id = ?1", [id])
                .map_err(to_string)?;
        }
        let slots =
            |ids: &[String]| -> Vec<usize> { ids.iter().map(|id| self.slots[id]).collect() };
        self.write_places(tx, &slots(&changes.moved))?;
        self.write_lists(tx, &slots(&changes.lists))
    }

    fn write_places(&self, tx: &Connection, slots: &[usize]) -> Result<(), String> {
        let mut put = tx
            .prepare_cached(
                "INSERT OR REPLACE INTO positions (id, hash, x, y) VALUES (?1, ?2, ?3, ?4)",
            )
            .map_err(to_string)?;
        for &slot in slots {
            let Some((id, hash)) = &self.notes[slot] else {
                continue;
            };
            let [x, y] = self.at[slot];
            put.execute(rusqlite::params![id, hash, x as f64, y as f64])
                .map_err(to_string)?;
        }
        Ok(())
    }

    fn write_lists(&self, tx: &Connection, slots: &[usize]) -> Result<(), String> {
        let mut clear = tx
            .prepare_cached("DELETE FROM links WHERE id = ?1")
            .map_err(to_string)?;
        let mut put = tx
            .prepare_cached("INSERT INTO links (id, other, score) VALUES (?1, ?2, ?3)")
            .map_err(to_string)?;
        for &slot in slots {
            let id = self.id(slot);
            clear.execute([id]).map_err(to_string)?;
            for &(score, to) in &self.near[slot] {
                put.execute(rusqlite::params![id, self.id(to), score as f64])
                    .map_err(to_string)?;
            }
        }
        Ok(())
    }
}

/// Bring the saved map in line with the vectors. The vectors are held only
/// while copied, so search goes on meanwhile. The map is loaded from `db`
/// first if it is not in memory yet. True when any note moved.
pub fn reconcile(
    vectors: &Mutex<Option<Vectors>>,
    map: &Mutex<Option<Map>>,
    db: &Mutex<Option<SpaceDb>>,
) -> Result<bool, String> {
    let held = vectors.lock().map_err(|_| "vectors lock poisoned")?;
    let Some(store) = held.as_ref() else {
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
    let current = slot.as_mut().expect("loaded just above");
    if !current.behind(store) {
        return Ok(false);
    }
    let snapshot = Snapshot::of(store);
    drop(held);

    let started = std::time::Instant::now();
    let afresh = current.model != snapshot.model || current.slots.is_empty();
    let changes = if afresh {
        current.lay_out(&snapshot);
        None
    } else {
        Some(current.update(&snapshot))
    };

    let mut db = db.lock().map_err(|_| "space.db lock poisoned")?;
    // The space was closed meanwhile.
    let Some(db) = db.as_mut() else {
        return Ok(false);
    };
    match &changes {
        None => {
            db.transaction(|tx| current.write_all(tx))?;
            log::info!(
                "laid out {} notes in {:?}",
                snapshot.len(),
                started.elapsed()
            );
        }
        Some(changes) => db.transaction(|tx| current.write_changes(tx, changes))?,
    }
    Ok(true)
}

/// The closest notes to `i` among `pool` by `score`, closest first. Kept as
/// it goes, so that comparing with thousands of notes allocates nothing.
fn top(i: usize, pool: &[usize], score: impl Fn(usize, usize) -> f32) -> Near {
    best(pool.iter().filter(|&&j| j != i).map(|&j| (score(i, j), j)))
}

/// The `LINKS` best of `scored`, closest first. Equal scores keep the order
/// they came in.
fn best(scored: impl IntoIterator<Item = (f32, usize)>) -> Near {
    let mut near: Near = Vec::with_capacity(LINKS + 1);
    for (s, j) in scored {
        if near.len() < LINKS || s > near[near.len() - 1].0 {
            let at = near.partition_point(|(b, _)| *b >= s);
            near.insert(at, (s, j));
            near.truncate(LINKS);
        }
    }
    near
}

/// The average of the places of the first `PLACE_AMONG` of `near`, weighted
/// by how close. The middle of the map when there are none.
fn average(near: &[(f32, usize)], y: &[[f32; 2]]) -> [f32; 2] {
    let among = &near[..PLACE_AMONG.min(near.len())];
    if among.is_empty() {
        return [0.0, 0.0];
    }
    let total: f32 = among.iter().map(|(s, _)| s.max(0.0)).sum();
    among.iter().fold([0.0, 0.0], |p, &(s, j)| {
        let w = if total > 0.0 {
            s.max(0.0) / total
        } else {
            1.0 / among.len() as f32
        };
        [p[0] + w * y[j][0], p[1] + w * y[j][1]]
    })
}

/// Every note's place in a full layout, and its closest notes.
fn layout(snapshot: &Snapshot) -> (Vec<[f32; 2]>, Vec<Near>) {
    let n = snapshot.len();
    let mut y = principal(snapshot);
    let near = closest(snapshot, LINKS);
    if n > 1 {
        let everyone: Vec<usize> = (0..n).collect();
        let movable = vec![true; n];
        pull_and_push(
            &mut y,
            &near,
            &everyone,
            &movable,
            &everyone,
            EPOCHS,
            1.0,
            &mut Random::new(),
        );
    }
    (y, near)
}

/// The `k` closest notes of every note. Every note against every other, so
/// split over the cores.
fn closest(snapshot: &Snapshot, k: usize) -> Vec<Near> {
    let n = snapshot.len();
    let everyone: Vec<usize> = (0..n).collect();
    let threads = std::thread::available_parallelism().map_or(1, |c| c.get());
    let chunk = n.div_ceil(threads).max(1);
    let mut out = vec![Vec::new(); n];
    std::thread::scope(|scope| {
        for (c, slice) in out.chunks_mut(chunk).enumerate() {
            let everyone = &everyone;
            scope.spawn(move || {
                for (offset, slot) in slice.iter_mut().enumerate() {
                    let mut near = top(c * chunk + offset, everyone, |a, b| snapshot.score(a, b));
                    near.truncate(k);
                    *slot = near;
                }
            });
        }
    });
    out
}

/// A fixed sequence, so a space lays out the same way every time.
struct Random(u64);

impl Random {
    fn new() -> Self {
        Self(0x9e37_79b9_7f4a_7c15)
    }

    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

/// Pull each of `moving` towards its closest notes and push it away from
/// random notes of `pool`, in rounds of shrinking steps, with UMAP's
/// gradients for a = b = 1. A note pulled along moves only if `movable`.
#[allow(clippy::too_many_arguments)]
fn pull_and_push(
    y: &mut [[f32; 2]],
    near: &[Near],
    moving: &[usize],
    movable: &[bool],
    pool: &[usize],
    epochs: usize,
    first_step: f32,
    random: &mut Random,
) {
    let clip = |g: f32| g.clamp(-4.0, 4.0);
    for epoch in 0..epochs {
        let step = first_step * (1.0 - epoch as f32 / epochs as f32);
        for &i in moving {
            for &(_, j) in &near[i] {
                let d = [y[i][0] - y[j][0], y[i][1] - y[j][1]];
                let q = -2.0 / (1.0 + d[0] * d[0] + d[1] * d[1]);
                for c in 0..2 {
                    let g = step * clip(q * d[c]);
                    y[i][c] += g;
                    if movable[j] {
                        y[j][c] -= g;
                    }
                }
                for _ in 0..PUSHES {
                    let r = pool[random.below(pool.len())];
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
        assert!(reconcile(&vectors, &map, &db).unwrap());
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
        assert!(!reconcile(&vectors, &map, &db).unwrap(), "nothing to do");
    }

    #[test]
    fn a_new_note_joins_its_topic_and_only_the_notes_near_it_move() {
        let (vectors, map, db) = slots(topics(4, 12, 16));
        reconcile(&vectors, &map, &db).unwrap();
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
        assert!(reconcile(&vectors, &map, &db).unwrap());

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
        reconcile(&vectors, &map, &db).unwrap();

        // A note deleted leaves every list, which takes the next closest.
        let present = (1..48)
            .map(|i| format!("{:03}{:02}", i / 4, i % 4))
            .collect();
        vectors.lock().unwrap().as_mut().unwrap().retain(&present);
        let before = places(&map);
        assert!(reconcile(&vectors, &map, &db).unwrap());
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
            assert!(reconcile(&vectors, &map, &db).unwrap());
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
        assert_eq!(saved(&db).model, "other");
    }

    #[test]
    fn one_note_or_none_is_placed_without_failing() {
        let (vectors, map, db) = slots(Vectors::new("m", 4));
        assert!(!reconcile(&vectors, &map, &db).unwrap());
        for id in ["01A", "01B"] {
            vectors
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .insert(id.into(), "h".into(), vec![1.0, 0.0, 0.0, 0.0])
                .unwrap();
            assert!(reconcile(&vectors, &map, &db).unwrap());
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
                let v = bytes
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .collect();
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
        reconcile(&vectors, &map, &db).unwrap();
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
            reconcile(&vectors, &map, &db).unwrap();
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
        pull_and_push(
            &mut y,
            &near,
            &everyone,
            &vec![true; n],
            &everyone,
            EPOCHS,
            1.0,
            &mut Random::new(),
        );
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
}
