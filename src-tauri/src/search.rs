//! Search, SPEC 6.
//!
//! A query is words. Every word must appear somewhere in a page's title or a
//! note's body. Matching ignores case and accents, so `reunion` finds
//! `Réunion`. The words are looked for in `search.db`, which holds the text;
//! the order and the stretch asked for come from the index in memory.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use crate::storage::daily_file::Note;
use crate::storage::index::{Index, IndexEntry};
use crate::storage::search_db::SearchDb;

/// Lowercase with accents stripped: decompose, then drop the combining marks.
pub fn fold(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Query {
    words: Vec<String>,
}

impl Query {
    fn parse(raw: &str) -> Self {
        Query {
            words: raw.split_whitespace().map(fold).collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// The notes whose text holds every word, or `None` for a query without
    /// words, which every note passes.
    fn matched(&self, db: &SearchDb) -> Result<Option<HashSet<String>>, String> {
        if self.words.is_empty() {
            return Ok(None);
        }
        db.matching(&self.words).map(Some)
    }
}

/// Whether `matched`, from `Query::matched`, lets `entry` through.
fn passes(matched: &Option<HashSet<String>>, entry: &IndexEntry) -> bool {
    matched.as_ref().map_or(true, |ids| ids.contains(&entry.id))
}

/// `entries` as notes, their text read from `db`.
pub fn with_bodies<'a>(
    db: &SearchDb,
    entries: impl IntoIterator<Item = &'a IndexEntry>,
) -> Result<Vec<Note>, String> {
    let entries: Vec<&IndexEntry> = entries.into_iter().collect();
    let mut bodies = db.bodies(entries.iter().map(|entry| entry.id.as_str()))?;
    Ok(entries
        .into_iter()
        .map(|entry| Note {
            body: bodies.remove(&entry.id).unwrap_or_default(),
            ..entry.to_note()
        })
        .collect())
}

fn newest_first(hits: &mut [&IndexEntry]) {
    hits.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.time.cmp(&a.time)));
}

/// Up to `k` notes of `hits`, best first, that do not match the query's
/// words: what search by meaning adds under the word matches.
pub fn by_meaning(
    index: &Index,
    db: &SearchDb,
    raw: &str,
    hits: &[(String, f32)],
    k: usize,
) -> Result<Vec<Note>, String> {
    let query = Query::parse(raw);
    let matched = query.matched(db)?;
    let entries: HashMap<&str, &IndexEntry> = index
        .entries()
        .map(|entry| (entry.id.as_str(), entry))
        .collect();
    let found: Vec<&IndexEntry> = hits
        .iter()
        .filter_map(|(id, _)| entries.get(id.as_str()).copied())
        .filter(|entry| !passes(&matched, entry))
        .take(k)
        .collect();
    with_bodies(db, found)
}

/// One stretch of what a search found, and how many notes it found in all.
#[derive(Debug, Default, Serialize)]
pub struct Found {
    pub notes: Vec<Note>,
    pub total: usize,
}

/// Newest first: by date, then by time within the day. Only the `limit`
/// notes from `offset` on are read out, bodies and all, so a short query
/// matching most of the space does not hand all of it to the page.
pub fn search(
    index: &Index,
    db: &SearchDb,
    raw: &str,
    offset: usize,
    limit: usize,
) -> Result<Found, String> {
    let query = Query::parse(raw);
    if query.is_empty() {
        return Ok(Found::default());
    }

    let matched = query.matched(db)?;
    let mut hits: Vec<&IndexEntry> = index
        .entries()
        .filter(|entry| passes(&matched, entry))
        .collect();
    newest_first(&mut hits);
    Ok(Found {
        total: hits.len(),
        notes: with_bodies(db, hits.into_iter().skip(offset).take(limit))?,
    })
}

/// Notes and pages whose body holds any of `needles` as typed, newest first.
/// A plugin asks for the markup it reads, `[ ]` and `[x]` for the tasks
/// view (SPEC 3.8), and parses the markdown itself to keep the real ones.
/// An empty needle matches every note.
pub fn containing(index: &Index, db: &SearchDb, needles: &[String]) -> Result<Vec<Note>, String> {
    let ids = db.containing(needles)?;
    let mut hits: Vec<&IndexEntry> = index
        .entries()
        .filter(|entry| ids.contains(&entry.id))
        .collect();
    newest_first(&mut hits);
    with_bodies(db, hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::daily_file::{body_hash, Kind};
    use crate::storage::relative_day_path;

    fn note(id: &str, date: &str, time: &str, body: &str) -> Note {
        Note {
            id: id.to_string(),
            date: date.to_string(),
            time: time.to_string(),
            file: relative_day_path(date),
            subject: None,
            hash: body_hash(body),
            on: None,
            ahead_off: false,
            body: body.to_string(),
            kind: Kind::Note,
            missing: false,
        }
    }

    /// Notes as an open space holds them: the index, and their text in
    /// search.db.
    struct Notes {
        index: Index,
        db: SearchDb,
    }

    fn index(notes: &[Note]) -> Notes {
        let mut index = Index::default();
        let mut db = SearchDb::in_memory().unwrap();
        db.at_once(|db| {
            for note in notes {
                index.push(IndexEntry::from(note));
                db.add_note(note, None).unwrap();
            }
        });
        Notes { index, db }
    }

    fn ids(hits: &[Note]) -> Vec<&str> {
        hits.iter().map(|n| n.id.as_str()).collect()
    }

    fn by_meaning_of(notes: &Notes, raw: &str, hits: &[(String, f32)], k: usize) -> Vec<Note> {
        by_meaning(&notes.index, &notes.db, raw, hits, k).unwrap()
    }

    fn containing_of(notes: &Notes, needles: &[String]) -> Vec<Note> {
        containing(&notes.index, &notes.db, needles).unwrap()
    }

    /// Every match, as a plugin's search asks for them.
    fn all(notes: &Notes, raw: &str) -> Vec<Note> {
        search(&notes.index, &notes.db, raw, 0, usize::MAX)
            .unwrap()
            .notes
    }

    #[test]
    fn a_stretch_of_the_matches_comes_with_their_total() {
        let idx = index(&[
            note("01A", "2026-09-21", "08:00", "x"),
            note("01B", "2026-09-22", "08:00", "x"),
            note("01C", "2026-09-22", "17:00", "x"),
            note("01D", "2026-09-23", "08:00", "y"),
        ]);
        let first = search(&idx.index, &idx.db, "x", 0, 2).unwrap();
        assert_eq!((ids(&first.notes), first.total), (vec!["01C", "01B"], 3));
        let rest = search(&idx.index, &idx.db, "x", 2, 2).unwrap();
        assert_eq!((ids(&rest.notes), rest.total), (vec!["01A"], 3));
        assert!(search(&idx.index, &idx.db, "x", 5, 2).unwrap().notes.is_empty());
    }

    #[test]
    fn folds_case_and_accents() {
        assert_eq!(fold("Réunion ÉQUIPE"), "reunion equipe");
    }

    #[test]
    fn splits_the_query_into_folded_words() {
        let query = Query::parse("Réunion  kubernetes");
        assert_eq!(query.words, vec!["reunion", "kubernetes"]);
    }

    #[test]
    fn an_empty_query_finds_nothing() {
        let idx = index(&[note("01A", "2026-09-22", "08:00", "anything")]);
        assert!(all(&idx, "   ").is_empty());
    }

    #[test]
    fn matches_substrings_ignoring_case_and_accents() {
        let idx = index(&[
            note("01A", "2026-09-22", "08:00", "Réunion demain"),
            note("01B", "2026-09-22", "09:00", "Buy coffee"),
        ]);
        assert_eq!(ids(&all(&idx, "REUNION")), vec!["01A"]);
        assert_eq!(ids(&all(&idx, "offe")), vec!["01B"]);
    }

    #[test]
    fn searches_a_page_s_title_too() {
        let mut n = note("01A", "2026-09-22", "08:00", "body text");
        n.subject = Some("Rollback plan".into());
        n.kind = Kind::Page;
        let idx = index(&[n]);
        assert_eq!(ids(&all(&idx, "rollback")), vec!["01A"]);
    }

    #[test]
    fn every_word_must_match() {
        let idx = index(&[
            note("01A", "2026-09-22", "08:00", "staging broke again"),
            note("01B", "2026-09-22", "09:00", "staging is fine"),
        ]);
        assert_eq!(ids(&all(&idx, "staging broke")), vec!["01A"]);
    }

    #[test]
    fn by_meaning_adds_what_the_words_missed() {
        let idx = index(&[
            note("01A", "2026-09-22", "08:00", "postgres index tip"),
            note("01B", "2026-09-22", "09:00", "slow query plan"),
            note("01C", "2026-09-22", "10:00", "sql migration"),
        ]);
        let hits: Vec<(String, f32)> = [("01C", 0.4), ("01Z", 0.35), ("01B", 0.3), ("01A", 0.2)]
            .into_iter()
            .map(|(id, score)| (id.to_string(), score))
            .collect();
        // 01A already matches the words, 01Z is not a note.
        assert_eq!(
            ids(&by_meaning_of(&idx, "postgres", &hits, 5)),
            vec!["01C", "01B"]
        );
        assert_eq!(ids(&by_meaning_of(&idx, "postgres", &hits, 1)), vec!["01C"]);
    }

    #[test]
    fn newest_first_by_date_then_time() {
        let idx = index(&[
            note("01A", "2026-09-21", "23:00", "x"),
            note("01B", "2026-09-22", "08:00", "x"),
            note("01C", "2026-09-22", "17:00", "x"),
        ]);
        assert_eq!(ids(&all(&idx, "x")), vec!["01C", "01B", "01A"]);
    }

    #[test]
    fn containing_keeps_notes_with_any_needle_newest_first() {
        let idx = index(&[
            note("01A", "2026-09-21", "08:00", "- [ ] call the bank"),
            note("01B", "2026-09-22", "08:00", "no tasks here"),
            note("01C", "2026-09-22", "09:00", "Shopping\n- [X] milk"),
        ]);
        let boxes = ["[ ]", "[x]", "[X]"].map(String::from);
        assert_eq!(ids(&containing_of(&idx, &boxes)), vec!["01C", "01A"]);
        assert_eq!(ids(&containing_of(&idx, &[String::new()])), vec!["01C", "01B", "01A"]);
        assert!(containing_of(&idx, &[]).is_empty());
    }

    #[test]
    fn results_carry_the_body_for_the_note_card() {
        let idx = index(&[note("01A", "2026-09-22", "08:00", "the body")]);
        assert_eq!(all(&idx, "body")[0].body, "the body");
    }

    /// SPEC 6: under 50ms for 10,000 notes. Debug builds are several times
    /// slower than release, so this is only a gate in release:
    /// `cargo test --release search_is_fast`.
    #[test]
    fn search_is_fast_enough_for_ten_thousand_notes() {
        let notes: Vec<Note> = (0..10_000)
            .map(|i| {
                let body = format!(
                    "Note number {i}. Talked with the team about the deployment \
                     pipeline, the staging cluster and the release on Friday. \
                     Réunion avec l'équipe infra pour le déploiement."
                );
                let date = format!("2026-{:02}-{:02}", 1 + i % 12, 1 + i % 28);
                note(&format!("{i:05}"), &date, "12:00", &body)
            })
            .collect();
        let idx = index(&notes);

        // As the command center asks, keystroke after keystroke.
        let started = std::time::Instant::now();
        let found = search(&idx.index, &idx.db, "deploiement friday", 0, 50).unwrap();
        let took = started.elapsed();

        assert_eq!((found.total, found.notes.len()), (10_000, 50));
        eprintln!("searched 10,000 notes in {took:?}");
        if !cfg!(debug_assertions) {
            assert!(took.as_millis() < 50, "took {took:?}");
        }
    }
}
