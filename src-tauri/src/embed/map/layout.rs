//! Laying the map out: a full layout pulls each note towards its closest
//! notes and pushes it away from others, as UMAP does, from the two
//! directions the notes differ most along; an update places each note new
//! or written again among its closest notes and lets them settle.

use std::collections::HashSet;

use super::{Changes, Map, Near};
use crate::embed::math::dot;
use crate::embed::vectors::Vectors;

/// The closest notes each note is pulled towards.
pub(super) const LINKS: usize = 15;
/// The closest notes a note new or written again starts among.
pub(super) const PLACE_AMONG: usize = 5;
/// Rounds of a full layout. UMAP's own default for a space this size.
pub(super) const EPOCHS: usize = 300;
/// Notes a note is pushed away from for each note it is pulled towards.
pub(super) const PUSHES: usize = 5;
/// Rounds a note new or written again settles for with the notes around
/// it, and the step they start at: UMAP's own for points added to a map.
pub(super) const SETTLE_EPOCHS: usize = 30;
pub(super) const SETTLE_STEP: f32 = 0.25;

/// What a layout reads from the vectors, copied so that they are let go
/// while it runs: ids in order, and every vector with the mean taken off,
/// at unit length.
pub(super) struct Snapshot {
    pub(super) model: String,
    pub(super) ids: Vec<String>,
    pub(super) hashes: Vec<String>,
    pub(super) dims: usize,
    pub(super) rows: Vec<f32>,
}

impl Snapshot {
    pub(super) fn of(vectors: &Vectors) -> Self {
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

    pub(super) fn len(&self) -> usize {
        self.ids.len()
    }

    fn row(&self, i: usize) -> &[f32] {
        &self.rows[i * self.dims..(i + 1) * self.dims]
    }

    fn score(&self, i: usize, j: usize) -> f32 {
        dot(self.row(i), self.row(j))
    }
}

impl Map {
    /// Lay every note out afresh. Its slots are the snapshot's order.
    pub(super) fn lay_out(&mut self, snapshot: &Snapshot) {
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
            categories: None,
        };
    }

    /// Forget the notes gone, and place the notes new or written again one
    /// by one, each settling with the notes around it.
    pub(super) fn update(&mut self, snapshot: &Snapshot) -> Changes {
        let (mut rows, mut placed) = self.rows(snapshot);
        let (out, gone) = self.forget_gone(snapshot, &rows, &mut placed);
        let mut lists: HashSet<usize> = HashSet::new();
        let mut pool = self.refill_lists(snapshot, &rows, &out, &mut lists);
        let moved = self.place_new(snapshot, &mut rows, &placed, &mut pool, &mut lists);

        let ids =
            |slots: HashSet<usize>| slots.into_iter().map(|s| self.id(s).to_string()).collect();
        Changes {
            moved: ids(moved),
            gone,
            lists: ids(lists),
        }
    }

    /// Which row of the snapshot each slot's note is, `usize::MAX` for one
    /// it no longer holds, and which rows are on the map already.
    fn rows(&self, snapshot: &Snapshot) -> (Vec<usize>, Vec<bool>) {
        let mut rows = vec![usize::MAX; self.notes.len()];
        let mut placed = vec![false; snapshot.len()];
        for (i, id) in snapshot.ids.iter().enumerate() {
            if let Some(&slot) = self.slots.get(id) {
                rows[slot] = i;
                placed[i] = true;
            }
        }
        (rows, placed)
    }

    /// Notes gone, and notes written again, leave the map and every list:
    /// their slots come free, and a note written again is no longer
    /// `placed`. Says which slots left, by slot, and the ids of the notes
    /// gone.
    fn forget_gone(
        &mut self,
        snapshot: &Snapshot,
        rows: &[usize],
        placed: &mut [bool],
    ) -> (Vec<bool>, Vec<String>) {
        let mut out = vec![false; self.notes.len()];
        let mut gone = Vec::new();
        for slot in 0..self.notes.len() {
            let Some((id, hash)) = &self.notes[slot] else {
                continue;
            };
            if rows[slot] == usize::MAX {
                gone.push(id.clone());
                out[slot] = true;
            } else if *hash != snapshot.hashes[rows[slot]] {
                placed[rows[slot]] = false;
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
        (out, gone)
    }

    /// Take the slots that left, `out`, off every list, and give a list
    /// that lost a note the next closest, noting it in `lists`. Returns the
    /// slots still on the map.
    fn refill_lists(
        &mut self,
        snapshot: &Snapshot,
        rows: &[usize],
        out: &[bool],
        lists: &mut HashSet<usize>,
    ) -> Vec<usize> {
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
        for &slot in &short {
            self.near[slot] = top(slot, &pool, |a, b| snapshot.score(rows[a], rows[b]));
            lists.insert(slot);
        }
        pool
    }

    /// Put each row not `placed` in a slot, among its closest notes, and let
    /// it and the notes around it settle. It joins `pool`, and the lists it
    /// changed go in `lists`. Returns the slots that moved.
    fn place_new(
        &mut self,
        snapshot: &Snapshot,
        rows: &mut Vec<usize>,
        placed: &[bool],
        pool: &mut Vec<usize>,
        lists: &mut HashSet<usize>,
    ) -> HashSet<usize> {
        let mut moved: HashSet<usize> = HashSet::new();
        let mut random = Random::new();
        for i in (0..snapshot.len()).filter(|&i| !placed[i]) {
            let slot = self.free.pop().unwrap_or_else(|| {
                self.notes.push(None);
                self.at.push([0.0, 0.0]);
                self.near.push(Vec::new());
                rows.push(usize::MAX);
                self.notes.len() - 1
            });
            rows[slot] = i;
            self.notes[slot] = Some((snapshot.ids[i].clone(), snapshot.hashes[i].clone()));
            self.slots.insert(snapshot.ids[i].clone(), slot);

            // It joins the lists of the notes it is closer to than their
            // last, and gets its own.
            let scored: Near = pool
                .iter()
                .map(|&j| (snapshot.score(i, rows[j]), j))
                .collect();
            let mut touched = Vec::new();
            for &(s, j) in &scored {
                if insert_ranked(&mut self.near[j], s, slot) {
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
            let settle = Settle {
                moving: &moving,
                movable: &movable,
                pool: pool.as_slice(),
                epochs: SETTLE_EPOCHS,
                first_step: SETTLE_STEP,
            };
            pull_and_push(&mut self.at, &self.near, &settle, &mut random);
            moved.extend(moving);
        }
        moved
    }
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
        insert_ranked(&mut near, s, j);
    }
    near
}

/// Put note `j`, `s` close, in `near`, a list kept closest first and at
/// most `LINKS` long, if it is closer than the list's last or the list is
/// short. Equal scores keep the order they came in. True when it went in.
fn insert_ranked(near: &mut Near, s: f32, j: usize) -> bool {
    if near.len() >= LINKS && s <= near[near.len() - 1].0 {
        return false;
    }
    let at = near.partition_point(|(b, _)| *b >= s);
    near.insert(at, (s, j));
    near.truncate(LINKS);
    true
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
        let settle = Settle::everyone(&everyone, &movable);
        pull_and_push(&mut y, &near, &settle, &mut Random::new());
    }
    (y, near)
}

/// The `k` closest notes of every note. Every note against every other, so
/// split over the cores.
pub(super) fn closest(snapshot: &Snapshot, k: usize) -> Vec<Near> {
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
pub(super) struct Random(u64);

impl Random {
    pub(super) fn new() -> Self {
        Self(0x9e37_79b9_7f4a_7c15)
    }

    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

/// The notes a round of `pull_and_push` moves, and how.
pub(super) struct Settle<'a> {
    /// The notes pulled and pushed.
    pub(super) moving: &'a [usize],
    /// By slot, whether a note pulled along moves.
    pub(super) movable: &'a [bool],
    /// The notes a note is pushed away from, at random.
    pub(super) pool: &'a [usize],
    pub(super) epochs: usize,
    /// The step of the first round, which shrinks to nothing by the last.
    pub(super) first_step: f32,
}

impl<'a> Settle<'a> {
    /// Every note, `everyone`, moving as a full layout moves them.
    pub(super) fn everyone(everyone: &'a [usize], movable: &'a [bool]) -> Self {
        Self {
            moving: everyone,
            movable,
            pool: everyone,
            epochs: EPOCHS,
            first_step: 1.0,
        }
    }
}

/// Pull each moving note towards its closest notes and push it away from
/// random notes of the pool, in rounds of shrinking steps, with UMAP's
/// gradients for a = b = 1.
pub(super) fn pull_and_push(
    y: &mut [[f32; 2]],
    near: &[Near],
    settle: &Settle,
    random: &mut Random,
) {
    let Settle {
        moving,
        movable,
        pool,
        epochs,
        first_step,
    } = *settle;
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
pub(super) fn principal(snapshot: &Snapshot) -> Vec<[f32; 2]> {
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
