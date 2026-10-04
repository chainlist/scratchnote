//! The categories of a space: groups of notes lying close together on its
//! map, each inside a bigger one, as the map shows them zoomed in and out
//! (SPEC 6.5). Saved in the `categories` and `category_notes` tables of
//! `space.db`.
//!
//! At level k, two notes are joined when they lie within `base` times √2^k
//! of each other on the map, and a group of 3 notes or more so joined is a
//! category. A group at one level lies inside one group at the next, so the
//! groups make a tree, up to the level where every note is in one. A
//! category is kept once for all the levels its notes stay the same, from
//! `low` to `high`. `base` is 0.8 times how far a note usually lies from
//! its fifth closest, set when the map is laid out whole, so that adding
//! notes never shifts the levels.
//!
//! Found again whenever the map changes. A category holding mostly the
//! notes one held before keeps its id. Each is named by the three words
//! that most set it apart from the whole map.

use std::collections::{BTreeSet, HashMap};

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::storage::space_db::{to_string, SpaceDb};

/// How close, against how far apart notes usually lie, two notes are
/// joined at the lowest level.
const CLOSE: f32 = 0.8;
/// The notes a category needs.
const LEAST: usize = 3;
/// A guard on the levels, should the notes never all come together.
const LEVELS: u32 = 64;
/// How much of its notes a category must share with one found before to
/// keep that one's id: more than half of the two together.
const SAME: f32 = 0.5;

/// A group of notes close together on the map.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Category {
    pub id: i64,
    /// The category it lies inside, `None` for the biggest.
    pub parent: Option<i64>,
    /// The first and last levels it shows at.
    pub low: u32,
    pub high: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Categories {
    /// How far apart two notes may lie to be joined at level 0.
    pub base: f32,
    /// Every category, by id.
    pub list: Vec<Category>,
    /// By note: the smallest category it is in. Its others are that one's
    /// parents. A note in none is left out.
    pub notes: HashMap<String, i64>,
}

/// A category as the tree is built, by place in the list.
struct Node {
    low: u32,
    high: u32,
    parent: Option<usize>,
    size: usize,
}

impl Categories {
    /// The saved categories, `None` when there are none yet.
    pub fn load(db: &SpaceDb) -> Option<Self> {
        let base = db.meta("categories_base")?.parse().ok()?;
        Self::read(db.conn(), base)
            .map_err(|e| log::warn!("could not read the categories: {e}"))
            .ok()
    }

    fn read(conn: &Connection, base: f32) -> rusqlite::Result<Self> {
        let mut statement =
            conn.prepare("SELECT id, parent, low, high, name FROM categories ORDER BY id")?;
        let list = statement
            .query_map([], |row| {
                Ok(Category {
                    id: row.get(0)?,
                    parent: row.get(1)?,
                    low: row.get(2)?,
                    high: row.get(3)?,
                    name: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut statement = conn.prepare("SELECT note, category FROM category_notes")?;
        let notes = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Self { base, list, notes })
    }

    /// The categories of the notes at `places`, by id. With `base` `None`,
    /// it is found from the places. Ids are kept from `before`, and `bodies`
    /// name them.
    pub fn find(
        places: &[(&str, [f32; 2])],
        base: Option<f32>,
        before: Option<&Self>,
        bodies: &HashMap<String, String>,
    ) -> Self {
        let at: Vec<[f32; 2]> = places.iter().map(|(_, at)| *at).collect();
        // Never under a ten-thousandth of the map, so that notes lying
        // mostly on top of each other still give a handful of levels.
        let extent = (0..2)
            .map(|c| {
                let values = at.iter().map(|p| p[c]);
                values.clone().fold(f32::NEG_INFINITY, f32::max)
                    - values.fold(f32::INFINITY, f32::min)
            })
            .fold(0.0, f32::max);
        let base = base
            .unwrap_or_else(|| CLOSE * spacing(&at))
            .max(extent * 1e-4)
            .max(f32::MIN_POSITIVE);
        let (nodes, leaf) = tree(&at, base);
        let mut members = vec![Vec::new(); nodes.len()];
        for (i, &first) in leaf.iter().enumerate() {
            let mut node = first;
            while let Some(n) = node {
                members[n].push(i);
                node = nodes[n].parent;
            }
        }
        let ids = keep_ids(places, &nodes, &leaf, &members, before);
        let names = names(places, &members, bodies);
        let mut list: Vec<Category> = nodes
            .iter()
            .zip(names)
            .enumerate()
            .map(|(n, (node, name))| Category {
                id: ids[n],
                parent: node.parent.map(|p| ids[p]),
                low: node.low,
                high: node.high,
                name,
            })
            .collect();
        list.sort_unstable_by_key(|category| category.id);
        Self {
            base,
            list,
            notes: places
                .iter()
                .zip(&leaf)
                .filter_map(|((id, _), node)| Some((id.to_string(), ids[(*node)?])))
                .collect(),
        }
    }

    /// Save these categories in place of `before`, writing only what
    /// changed.
    pub fn write(&self, tx: &Connection, before: Option<&Self>) -> Result<(), String> {
        let empty = Self::default();
        let before = before.unwrap_or_else(|| {
            let _ = tx.execute_batch("DELETE FROM categories; DELETE FROM category_notes;");
            &empty
        });
        SpaceDb::set_meta(tx, "categories_base", &self.base.to_string())?;
        let now: HashMap<i64, &Category> = self.list.iter().map(|c| (c.id, c)).collect();
        let mut gone = tx
            .prepare_cached("DELETE FROM categories WHERE id = ?1")
            .map_err(to_string)?;
        for old in &before.list {
            if !now.contains_key(&old.id) {
                gone.execute([old.id]).map_err(to_string)?;
            }
        }
        let mut put = tx
            .prepare_cached(
                "INSERT OR REPLACE INTO categories (id, parent, low, high, name)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .map_err(to_string)?;
        let then: HashMap<i64, &Category> = before.list.iter().map(|c| (c.id, c)).collect();
        for category in &self.list {
            if then.get(&category.id) != Some(&category) {
                put.execute(params![
                    category.id,
                    category.parent,
                    category.low,
                    category.high,
                    category.name
                ])
                .map_err(to_string)?;
            }
        }
        let mut gone = tx
            .prepare_cached("DELETE FROM category_notes WHERE note = ?1")
            .map_err(to_string)?;
        for note in before.notes.keys() {
            if !self.notes.contains_key(note) {
                gone.execute([note]).map_err(to_string)?;
            }
        }
        let mut put = tx
            .prepare_cached(
                "INSERT OR REPLACE INTO category_notes (note, category) VALUES (?1, ?2)",
            )
            .map_err(to_string)?;
        for (note, &category) in &self.notes {
            if before.notes.get(note) != Some(&category) {
                put.execute(params![note, category]).map_err(to_string)?;
            }
        }
        Ok(())
    }
}

/// How far apart notes usually lie on the map: the median distance from a
/// note to its fifth closest, over 500 notes at most. Its closest of all
/// in a space of fewer than 6 notes.
fn spacing(at: &[[f32; 2]]) -> f32 {
    let n = at.len();
    if n < 2 {
        return 0.0;
    }
    let k = 5.min(n - 1);
    let step = (n / 500).max(1);
    let mut kth: Vec<f32> = (0..n)
        .step_by(step)
        .map(|i| {
            let mut closest = [f32::INFINITY; 5];
            for j in (0..n).filter(|&j| j != i) {
                let d = distance2(at[i], at[j]);
                if d >= closest[k - 1] {
                    continue;
                }
                let mut place = k - 1;
                while place > 0 && closest[place - 1] > d {
                    closest[place] = closest[place - 1];
                    place -= 1;
                }
                closest[place] = d;
            }
            closest[k - 1].sqrt()
        })
        .collect();
    kth.sort_by(f32::total_cmp);
    kth[kth.len() / 2]
}

fn distance2(a: [f32; 2], b: [f32; 2]) -> f32 {
    let (x, y) = (a[0] - b[0], a[1] - b[1]);
    x * x + y * y
}

/// The groups of at least `LEAST` notes, a note in a group when it lies
/// within `reach` of another of its notes.
fn groups(at: &[[f32; 2]], reach: f32) -> Vec<Vec<usize>> {
    let n = at.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    // Notes by square, small enough that the notes of one square all lie
    // within reach of each other: each square is one group to start with.
    let side = reach / std::f32::consts::SQRT_2;
    let left = at.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let top = at.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let bottom = at.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
    // A square's key counts its column, then its row, with two rows free
    // above and below the notes so the squares near one never wrap round.
    let rows = ((bottom - top) / side) as i64 + 5;
    let mut squares: HashMap<i64, Vec<usize>> = HashMap::new();
    for (i, p) in at.iter().enumerate() {
        let column = ((p[0] - left) / side) as i64;
        let key = column * rows + ((p[1] - top) / side) as i64 + 2;
        let square = squares.entry(key).or_default();
        if let Some(&first) = square.first() {
            parent[i] = first;
        }
        square.push(i);
    }
    // Notes within reach lie at most two squares apart. Each pair of squares
    // once, and only until one note of each is found within reach.
    let mut ahead = Vec::new();
    for dx in 0..=2 {
        for dy in -2..=2 {
            if dx > 0 || dy > 0 {
                ahead.push(dx * rows + dy);
            }
        }
    }
    let reach2 = reach * reach;
    for (key, square) in &squares {
        for step in &ahead {
            let Some(other) = squares.get(&(key + step)) else {
                continue;
            };
            if root(&mut parent, square[0]) == root(&mut parent, other[0]) {
                continue;
            }
            'near: for &i in square {
                for &j in other {
                    if distance2(at[i], at[j]) <= reach2 {
                        let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                        parent[a] = b;
                        break 'near;
                    }
                }
            }
        }
    }
    let mut found: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        found.entry(root(&mut parent, i)).or_default().push(i);
    }
    let mut found: Vec<Vec<usize>> = found
        .into_values()
        .filter(|group| group.len() >= LEAST)
        .collect();
    // In the order of their first note, so the tree comes out the same.
    found.sort_unstable_by_key(|group| group[0]);
    found
}

/// The categories, smallest first, and by note its smallest, found level
/// by level until one holds every note.
fn tree(at: &[[f32; 2]], base: f32) -> (Vec<Node>, Vec<Option<usize>>) {
    let n = at.len();
    let mut nodes: Vec<Node> = Vec::new();
    let mut leaf = vec![None; n];
    if n < LEAST {
        return (nodes, leaf);
    }
    // By note, its category at the level before.
    let mut current: Vec<Option<usize>> = vec![None; n];
    for level in 0..LEVELS {
        let found = groups(at, base * std::f32::consts::SQRT_2.powi(level as i32));
        let whole = found.len() == 1 && found[0].len() == n;
        for group in found {
            let mut under: Vec<usize> = group.iter().filter_map(|&i| current[i]).collect();
            under.sort_unstable();
            under.dedup();
            // The same notes as one category at the level before: that
            // category goes on.
            if let [only] = under[..] {
                if nodes[only].size == group.len() {
                    nodes[only].high = level;
                    continue;
                }
            }
            let node = nodes.len();
            nodes.push(Node {
                low: level,
                high: level,
                parent: None,
                size: group.len(),
            });
            for child in under {
                nodes[child].parent = Some(node);
            }
            for i in group {
                leaf[i].get_or_insert(node);
                current[i] = Some(node);
            }
        }
        if whole {
            break;
        }
    }
    (nodes, leaf)
}

/// An id for each category: that of the category found before which it
/// shares most of its notes with, when that is more than half of the two
/// together, each only once, and new ids after the highest for the rest.
fn keep_ids(
    places: &[(&str, [f32; 2])],
    nodes: &[Node],
    leaf: &[Option<usize>],
    members: &[Vec<usize>],
    before: Option<&Categories>,
) -> Vec<i64> {
    let mut ids = vec![0; nodes.len()];
    let Some(before) = before else {
        (1..).zip(&mut ids).for_each(|(id, slot)| *slot = id);
        return ids;
    };
    let old: HashMap<i64, &Category> = before.list.iter().map(|c| (c.id, c)).collect();
    let chain = |first: Option<i64>| {
        let mut out = Vec::new();
        let mut at = first.and_then(|id| old.get(&id));
        while let Some(category) = at {
            out.push(*category);
            at = category.parent.and_then(|id| old.get(&id));
        }
        out
    };
    // How many notes each pair of an old and a new category shows at one
    // level shares. Both chains rise level by level, so they are walked
    // side by side.
    let mut shared: HashMap<(i64, usize), usize> = HashMap::new();
    let mut sizes: HashMap<i64, usize> = HashMap::new();
    for (i, (id, _)) in places.iter().enumerate() {
        let olds = chain(before.notes.get(*id).copied());
        for category in &olds {
            *sizes.entry(category.id).or_default() += 1;
        }
        let mut news = Vec::new();
        let mut at = leaf[i];
        while let Some(n) = at {
            news.push(n);
            at = nodes[n].parent;
        }
        let (mut a, mut b) = (0, 0);
        while a < olds.len() && b < news.len() {
            let (o, n) = (olds[a], &nodes[news[b]]);
            if o.low.max(n.low) <= o.high.min(n.high) {
                *shared.entry((o.id, news[b])).or_default() += 1;
            }
            if o.high < n.high {
                a += 1;
            } else {
                b += 1;
            }
        }
    }
    let mut pairs: Vec<(f32, i64, usize)> = shared
        .into_iter()
        .map(|((old, new), both)| {
            let either = sizes[&old] + members[new].len() - both;
            (both as f32 / either as f32, old, new)
        })
        .filter(|&(same, _, _)| same > SAME)
        .collect();
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut taken = std::collections::HashSet::new();
    let mut kept = vec![None; nodes.len()];
    for (_, old, new) in pairs {
        if kept[new].is_none() && taken.insert(old) {
            kept[new] = Some(old);
        }
    }
    let mut next = before.list.iter().map(|c| c.id).max().unwrap_or(0) + 1;
    for (slot, kept) in ids.iter_mut().zip(kept) {
        *slot = kept.unwrap_or_else(|| {
            next += 1;
            next - 1
        });
    }
    ids
}

/// The words of three letters or more of each note, lowercased, that can
/// tell one category from another: in two notes at least, so they say what
/// notes share, and in a quarter of them at most, as words that common say
/// nothing of one category. Each word as a place in the list returned.
fn words_of(
    places: &[(&str, [f32; 2])],
    bodies: &HashMap<String, String>,
) -> (Vec<String>, Vec<Vec<u32>>) {
    let held: Vec<BTreeSet<String>> = places
        .iter()
        .map(|(id, _)| {
            bodies.get(*id).map_or_else(BTreeSet::new, |body| {
                body.split(|c: char| !c.is_alphabetic())
                    .filter(|word| word.chars().count() >= 3)
                    .map(str::to_lowercase)
                    .collect()
            })
        })
        .collect();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for word in held.iter().flatten() {
        *counts.entry(word).or_default() += 1;
    }
    let mut words: Vec<&str> = counts
        .into_iter()
        .filter(|&(_, count)| count >= 2 && count * 4 <= places.len())
        .map(|(word, _)| word)
        .collect();
    words.sort_unstable();
    let place: HashMap<&str, u32> = (0..).zip(&words).map(|(i, word)| (*word, i)).collect();
    let notes = held
        .iter()
        .map(|held| {
            held.iter()
                .filter_map(|word| place.get(word.as_str()).copied())
                .collect()
        })
        .collect();
    (words.into_iter().map(str::to_string).collect(), notes)
}

/// Each category's name: the three words that most set it apart, held by a
/// tenth of its notes at least, and twice as often as across the map,
/// ranked by how many of its notes hold them and how much more often.
fn names(
    places: &[(&str, [f32; 2])],
    members: &[Vec<usize>],
    bodies: &HashMap<String, String>,
) -> Vec<String> {
    let (words, held) = words_of(places, bodies);
    let mut all = vec![0u32; words.len()];
    held.iter().flatten().for_each(|&w| all[w as usize] += 1);
    let total = places.len() as f32;
    // Counted in one list for every category, cleared after each.
    let mut count = vec![0u32; words.len()];
    let mut seen = Vec::new();
    members
        .iter()
        .map(|notes| {
            for &i in notes {
                for &w in &held[i] {
                    if count[w as usize] == 0 {
                        seen.push(w);
                    }
                    count[w as usize] += 1;
                }
            }
            let size = notes.len() as f32;
            let mut ranked: Vec<(f32, u32)> = seen
                .iter()
                .filter_map(|&w| {
                    let inside = count[w as usize] as f32 / size;
                    let lift = inside / (all[w as usize] as f32 / total);
                    (count[w as usize] >= 2 && inside >= 0.1 && lift >= 2.0)
                        .then(|| (inside * lift.ln(), w))
                })
                .collect();
            for w in seen.drain(..) {
                count[w as usize] = 0;
            }
            ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            ranked
                .iter()
                .take(3)
                .map(|&(_, w)| words[w as usize].as_str())
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two places far apart, each with two clumps of notes a little apart,
    /// each clump of `each` notes in a row. Ids say place, clump and note.
    fn clumps(each: usize) -> Vec<(String, [f32; 2])> {
        let mut out = Vec::new();
        for (p, px) in [(0, 0.0), (1, 100.0)] {
            for (c, cx) in [(0, 0.0), (1, 10.0)] {
                for i in 0..each {
                    out.push((format!("{p}{c}{i:02}"), [px + cx + i as f32, 0.0]));
                }
            }
        }
        out
    }

    fn places(notes: &[(String, [f32; 2])]) -> Vec<(&str, [f32; 2])> {
        notes.iter().map(|(id, at)| (id.as_str(), *at)).collect()
    }

    /// The categories a note is in, smallest first.
    fn chain(categories: &Categories, note: &str) -> Vec<i64> {
        let by_id: HashMap<i64, &Category> = categories.list.iter().map(|c| (c.id, c)).collect();
        let mut out = Vec::new();
        let mut at = categories.notes.get(note).copied();
        while let Some(id) = at {
            out.push(id);
            at = by_id[&id].parent;
        }
        out
    }

    #[test]
    fn clumps_nest_in_places_and_places_in_the_whole_map() {
        let notes = clumps(4);
        let found = Categories::find(&places(&notes), Some(1.0), None, &HashMap::new());
        // Each clump, then each place, then everything.
        assert_eq!(found.list.len(), 4 + 2 + 1);
        let a = chain(&found, "0000");
        let b = chain(&found, "0103");
        let c = chain(&found, "1000");
        assert_eq!(a.len(), 3);
        assert_ne!(a[0], b[0], "two clumps");
        assert_eq!(a[1..], b[1..], "in one place");
        assert_ne!(a[1], c[1], "two places");
        assert_eq!(a[2], c[2], "in one map");
        // Levels follow on from one category to the one it lies inside.
        let by_id: HashMap<i64, &Category> = found.list.iter().map(|c| (c.id, c)).collect();
        for pair in a.windows(2) {
            assert_eq!(by_id[&pair[0]].high + 1, by_id[&pair[1]].low);
        }
        assert_eq!(by_id[&a[0]].low, 0);
        assert_eq!(by_id[&a[2]].parent, None);
    }

    #[test]
    fn notes_too_few_or_far_from_others_are_in_no_category() {
        let two = [("a".to_string(), [0.0, 0.0]), ("b".to_string(), [1.0, 0.0])];
        let found = Categories::find(&places(&two), None, None, &HashMap::new());
        assert!(found.list.is_empty() && found.notes.is_empty());

        // A note on its own only joins at the level that reaches it, 40
        // away: a category of its place and itself, from level 11.
        let mut notes = clumps(4);
        notes.push(("lone".into(), [5.0, 40.0]));
        let found = Categories::find(&places(&notes), Some(1.0), None, &HashMap::new());
        let lone = chain(&found, "lone");
        assert_eq!(lone.len(), 2);
        assert_eq!(found.list.iter().find(|c| c.id == lone[0]).unwrap().low, 11);
        assert_eq!(chain(&found, "0000")[2..], lone[..]);
    }

    #[test]
    fn a_note_added_keeps_the_ids_and_the_levels() {
        let mut notes = clumps(4);
        let before = Categories::find(&places(&notes), None, None, &HashMap::new());
        notes.push(("0004".into(), [4.0, 0.0]));
        let after = Categories::find(
            &places(&notes),
            Some(before.base),
            Some(&before),
            &HashMap::new(),
        );
        assert_eq!(after.base, before.base);
        assert_eq!(chain(&after, "0000"), chain(&before, "0000"));
        assert_eq!(chain(&after, "0004"), chain(&before, "0000"));
        assert_eq!(chain(&after, "1100"), chain(&before, "1100"));

        // A clump that parts: the part with most of its notes keeps its id,
        // the other gets a new one.
        let moved: Vec<(String, [f32; 2])> = clumps(8)
            .into_iter()
            .map(|(id, [x, y])| (id, [x, if x < 3.0 { 5.0 } else { y }]))
            .collect();
        let start = Categories::find(&places(&clumps(8)), Some(1.0), None, &HashMap::new());
        let parted = Categories::find(&places(&moved), Some(1.0), Some(&start), &HashMap::new());
        let old_max = start.list.iter().map(|c| c.id).max().unwrap();
        assert_eq!(chain(&parted, "0007")[0], chain(&start, "0007")[0]);
        assert!(chain(&parted, "0000")[0] > old_max);
    }

    #[test]
    fn a_category_is_named_by_the_words_that_set_it_apart() {
        let notes = clumps(4);
        let bodies: HashMap<String, String> = notes
            .iter()
            .map(|(id, _)| {
                let about = match &id[..2] {
                    "00" => "Réparé the bike",
                    "01" => "The bike chain",
                    "10" => "The oven",
                    _ => "Oven timer",
                };
                (id.clone(), format!("{about}, again"))
            })
            .collect();
        let found = Categories::find(&places(&notes), Some(1.0), None, &bodies);
        let name = |id: i64| found.list.iter().find(|c| c.id == id).unwrap().name.clone();
        let first = chain(&found, "0000");
        // "the" and "again" are in most notes, "bike" and "oven" in half:
        // too common to name anything.
        assert_eq!(name(first[0]), "réparé");
        assert_eq!(name(first[1]), "chain · réparé");
        assert_eq!(name(first[2]), "", "nothing sets the whole map apart");
        assert_eq!(name(chain(&found, "1100")[0]), "timer");
    }

    #[test]
    fn categories_are_saved_and_read_back_as_they_change() {
        let mut db = SpaceDb::in_memory().unwrap();
        let mut notes = clumps(4);
        let first = Categories::find(&places(&notes), None, None, &HashMap::new());
        db.transaction(|tx| first.write(tx, None)).unwrap();
        assert_eq!(Categories::load(&db), Some(first.clone()));

        notes.retain(|(id, _)| !id.starts_with("10"));
        let second = Categories::find(
            &places(&notes),
            Some(first.base),
            Some(&first),
            &HashMap::new(),
        );
        db.transaction(|tx| second.write(tx, Some(&first))).unwrap();
        assert_eq!(Categories::load(&db), Some(second));
    }

    /// On a real space: how long finding, naming and saving the categories
    /// takes, from scratch and again after one note moves, and the biggest
    /// categories at a few levels. Reads the map from the space's
    /// `space.db` and the text from its `search.db`, never writes either.
    /// In release: `SCRATCHNOTE_BENCH_SPACE=<space folder> cargo test
    /// --release bench_categories -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bench_categories() {
        use rusqlite::OpenFlags;
        use std::time::Instant;
        let root = std::path::PathBuf::from(std::env::var("SCRATCHNOTE_BENCH_SPACE").unwrap());
        let open = |name: &str| {
            Connection::open_with_flags(
                root.join(".scratchnote").join(name),
                OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .unwrap()
        };
        let mut notes: Vec<(String, [f32; 2])> = open("space.db")
            .prepare("SELECT id, x, y FROM positions ORDER BY id")
            .unwrap()
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    [r.get::<_, f64>(1)? as f32, r.get::<_, f64>(2)? as f32],
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let started = Instant::now();
        let bodies: HashMap<String, String> = open("search.db")
            .prepare("SELECT id, body FROM texts")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        println!(
            "{} notes, text read in {:?}",
            notes.len(),
            started.elapsed()
        );

        let started = Instant::now();
        let first = Categories::find(&places(&notes), None, None, &bodies);
        let finding = started.elapsed();
        let started = Instant::now();
        let unnamed = Categories::find(&places(&notes), None, None, &HashMap::new());
        let unnamed_time = started.elapsed();
        let top = first.list.iter().map(|c| c.high).max().unwrap_or(0);
        println!(
            "found {} categories over levels 0 to {top}, base {:.3}, in {finding:?} ({unnamed_time:?} of it without names); {} notes in one",
            first.list.len(),
            first.base,
            first.notes.len()
        );
        assert_eq!(unnamed.list.len(), first.list.len());

        let mut db = SpaceDb::in_memory().unwrap();
        let started = Instant::now();
        db.transaction(|tx| first.write(tx, None)).unwrap();
        println!("saved whole in {:?}", started.elapsed());

        // One note moves a little, as when a note nearby is saved.
        let middle = notes.len() / 2;
        notes[middle].1[0] += first.base;
        let started = Instant::now();
        let again = Categories::find(&places(&notes), Some(first.base), Some(&first), &bodies);
        let finding = started.elapsed();
        let started = Instant::now();
        db.transaction(|tx| again.write(tx, Some(&first))).unwrap();
        let kept = again
            .list
            .iter()
            .filter(|c| first.list.iter().any(|f| f.id == c.id))
            .count();
        println!(
            "one note moved: found again in {finding:?}, saved in {:?}, {kept} of {} ids kept",
            started.elapsed(),
            again.list.len()
        );

        let sizes = {
            let by_id: HashMap<i64, &Category> = again.list.iter().map(|c| (c.id, c)).collect();
            let mut sizes: HashMap<i64, usize> = HashMap::new();
            for &leaf in again.notes.values() {
                let mut at = Some(leaf);
                while let Some(id) = at {
                    *sizes.entry(id).or_default() += 1;
                    at = by_id[&id].parent;
                }
            }
            sizes
        };
        for level in [0, top / 4, top / 2, 3 * top / 4, top] {
            let mut shown: Vec<&Category> = again
                .list
                .iter()
                .filter(|c| c.low <= level && level <= c.high)
                .collect();
            shown.sort_by_key(|c| std::cmp::Reverse(sizes[&c.id]));
            println!("level {level}: {} categories", shown.len());
            for c in shown.iter().take(8) {
                println!("  {:>5}  {}", sizes[&c.id], c.name);
            }
        }
    }
}
