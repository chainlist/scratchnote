//! In-memory search, SPEC 6.
//!
//! A query is words and `#category` tokens. Every word must appear somewhere
//! in the subject or body, and the note must be filed under the category.
//! Matching ignores case and accents, so `reunion` finds `Réunion`.

use std::collections::HashMap;

use serde::Serialize;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use crate::enrich::normalize;
use crate::storage::daily_file::Note;
use crate::storage::index::{Index, IndexEntry};

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
    categories: Vec<String>,
}

impl Query {
    /// Category tokens go through the same cleaning as a category typed in
    /// the editor, so `#Home Lab` is not needed to find `home-lab`.
    fn parse(raw: &str) -> Self {
        let mut query = Query::default();
        for token in raw.split_whitespace() {
            if token.starts_with('#') {
                if let Some(category) = normalize::clean(token) {
                    query.categories.push(fold(&category));
                }
            } else {
                query.words.push(fold(token));
            }
        }
        query
    }

    fn is_empty(&self) -> bool {
        self.words.is_empty() && self.categories.is_empty()
    }

    fn has_category(&self, entry: &IndexEntry) -> bool {
        self.categories
            .iter()
            .all(|wanted| entry.category.as_deref().map(fold).as_ref() == Some(wanted))
    }

    fn has_words(&self, entry: &IndexEntry) -> bool {
        self.words.iter().all(|word| entry.folded.contains(word))
    }
}

/// A query's words as typed, without its `#category` filter: what search by
/// meaning embeds.
pub fn words(raw: &str) -> String {
    raw.split_whitespace()
        .filter(|token| !token.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Up to `k` notes of `hits`, best first, that pass the query's `#category`
/// filter without matching its words: what search by meaning adds under the
/// word matches.
pub fn by_meaning(index: &Index, raw: &str, hits: &[(String, f32)], k: usize) -> Vec<Note> {
    let query = Query::parse(raw);
    let entries: HashMap<&str, &IndexEntry> = index
        .entries()
        .map(|entry| (entry.id.as_str(), entry))
        .collect();
    hits.iter()
        .filter_map(|(id, _)| entries.get(id.as_str()))
        .filter(|entry| query.has_category(entry) && !query.has_words(entry))
        .take(k)
        .map(|entry| entry.to_note())
        .collect()
}

/// One stretch of what a search found, and how many notes it found in all.
#[derive(Debug, Default, Serialize)]
pub struct Found {
    pub notes: Vec<Note>,
    pub total: usize,
}

/// Newest first: by date, then by time within the day. Only the `limit`
/// notes from `offset` on are copied out, bodies and all, so a short query
/// matching most of the space does not hand all of it to the page.
pub fn search(index: &Index, raw: &str, offset: usize, limit: usize) -> Found {
    let query = Query::parse(raw);
    if query.is_empty() {
        return Found::default();
    }

    let mut hits: Vec<&IndexEntry> = index
        .entries()
        .filter(|entry| query.has_category(entry) && query.has_words(entry))
        .collect();

    hits.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.time.cmp(&a.time)));
    Found {
        total: hits.len(),
        notes: hits
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(IndexEntry::to_note)
            .collect(),
    }
}

/// Notes and pages whose body holds any of `needles` as typed, newest first.
/// A plugin asks for the markup it reads, `[ ]` and `[x]` for the tasks
/// view (SPEC 3.8), and parses the markdown itself to keep the real ones.
/// An empty needle matches every note.
pub fn containing(index: &Index, needles: &[String]) -> Vec<Note> {
    let mut hits: Vec<Note> = index
        .entries()
        .filter(|entry| needles.iter().any(|needle| entry.body.contains(needle.as_str())))
        .map(|entry| entry.to_note())
        .collect();

    hits.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.time.cmp(&a.time)));
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::daily_file::{body_hash, Kind, Status};
    use crate::storage::relative_day_path;

    fn note(id: &str, date: &str, time: &str, body: &str, category: Option<&str>) -> Note {
        Note {
            id: id.to_string(),
            date: date.to_string(),
            time: time.to_string(),
            file: relative_day_path(date),
            subject: None,
            category: category.map(str::to_string),
            status: Status::Done,
            hash: body_hash(body),
            lang: None,
            body: body.to_string(),
            kind: Kind::Note,
            missing: false,
        }
    }

    fn index(notes: &[Note]) -> Index {
        let mut index = Index::default();
        for note in notes {
            index.push(IndexEntry::from(note));
        }
        index
    }

    fn ids(hits: &[Note]) -> Vec<&str> {
        hits.iter().map(|n| n.id.as_str()).collect()
    }

    /// Every match, as a plugin's search asks for them.
    fn all(index: &Index, raw: &str) -> Vec<Note> {
        search(index, raw, 0, usize::MAX).notes
    }

    #[test]
    fn a_stretch_of_the_matches_comes_with_their_total() {
        let idx = index(&[
            note("01A", "2026-09-21", "08:00", "x", None),
            note("01B", "2026-09-22", "08:00", "x", None),
            note("01C", "2026-09-22", "17:00", "x", None),
            note("01D", "2026-09-23", "08:00", "y", None),
        ]);
        let first = search(&idx, "x", 0, 2);
        assert_eq!((ids(&first.notes), first.total), (vec!["01C", "01B"], 3));
        let rest = search(&idx, "x", 2, 2);
        assert_eq!((ids(&rest.notes), rest.total), (vec!["01A"], 3));
        assert!(search(&idx, "x", 5, 2).notes.is_empty());
    }

    #[test]
    fn folds_case_and_accents() {
        assert_eq!(fold("Réunion ÉQUIPE"), "reunion equipe");
    }

    #[test]
    fn splits_words_from_categories() {
        let query = Query::parse("#Infra  kubernetes");
        assert_eq!(query.words, vec!["kubernetes"]);
        assert_eq!(query.categories, vec!["infra"]);
    }

    #[test]
    fn an_empty_query_finds_nothing() {
        let idx = index(&[note("01A", "2026-09-22", "08:00", "anything", None)]);
        assert!(all(&idx, "   ").is_empty());
        assert!(all(&idx, "#").is_empty());
    }

    #[test]
    fn matches_substrings_ignoring_case_and_accents() {
        let idx = index(&[
            note("01A", "2026-09-22", "08:00", "Réunion demain", None),
            note("01B", "2026-09-22", "09:00", "Buy coffee", None),
        ]);
        assert_eq!(ids(&all(&idx, "REUNION")), vec!["01A"]);
        assert_eq!(ids(&all(&idx, "offe")), vec!["01B"]);
    }

    #[test]
    fn searches_the_subject_too() {
        let mut n = note("01A", "2026-09-22", "08:00", "body text", None);
        n.subject = Some("Rollback plan".into());
        let idx = index(&[n]);
        assert_eq!(ids(&all(&idx, "rollback")), vec!["01A"]);
    }

    #[test]
    fn every_word_must_match() {
        let idx = index(&[
            note("01A", "2026-09-22", "08:00", "staging broke again", None),
            note("01B", "2026-09-22", "09:00", "staging is fine", None),
        ]);
        assert_eq!(ids(&all(&idx, "staging broke")), vec!["01A"]);
    }

    #[test]
    fn a_category_filters_on_the_notes_filed_under_it() {
        let idx = index(&[
            note("01A", "2026-09-22", "08:00", "a", Some("infra")),
            note("01B", "2026-09-22", "09:00", "b", Some("infra")),
            note("01C", "2026-09-22", "10:00", "c", Some("movie")),
            note("01D", "2026-09-22", "11:00", "d", None),
        ]);
        assert_eq!(ids(&all(&idx, "#infra")), vec!["01B", "01A"]);
        // A note has one category, so two never match together.
        assert!(all(&idx, "#infra #movie").is_empty());
    }

    #[test]
    fn a_category_must_match_whole_not_as_a_substring() {
        let idx = index(&[note("01A", "2026-09-22", "08:00", "a", Some("infrastructure"))]);
        assert!(all(&idx, "#infra").is_empty());
    }

    #[test]
    fn meaning_embeds_the_words_as_typed_without_the_category() {
        assert_eq!(words("#db  Slow queries"), "Slow queries");
        assert_eq!(words("#db"), "");
    }

    #[test]
    fn by_meaning_adds_what_the_words_missed_within_the_category() {
        let idx = index(&[
            note("01A", "2026-09-22", "08:00", "postgres index tip", Some("db")),
            note("01B", "2026-09-22", "09:00", "slow query plan", Some("db")),
            note("01C", "2026-09-22", "10:00", "sql migration", Some("infra")),
        ]);
        let hits: Vec<(String, f32)> = [("01C", 0.4), ("01Z", 0.35), ("01B", 0.3), ("01A", 0.2)]
            .into_iter()
            .map(|(id, score)| (id.to_string(), score))
            .collect();
        // 01A already matches the words, 01C is filed elsewhere, 01Z is not a note.
        assert_eq!(ids(&by_meaning(&idx, "postgres #db", &hits, 5)), vec!["01B"]);
        assert_eq!(ids(&by_meaning(&idx, "postgres", &hits, 1)), vec!["01C"]);
    }

    #[test]
    fn combines_a_category_and_words() {
        let idx = index(&[
            note("01A", "2026-09-22", "08:00", "kubernetes upgrade", Some("infra")),
            note("01B", "2026-09-22", "09:00", "kubernetes talk", Some("meetup")),
        ]);
        assert_eq!(ids(&all(&idx, "#infra kubernetes")), vec!["01A"]);
    }

    #[test]
    fn category_tokens_are_cleaned_and_ignore_accents() {
        let idx = index(&[
            note("01A", "2026-09-22", "08:00", "a", Some("home-lab")),
            note("01B", "2026-09-22", "09:00", "b", Some("série")),
        ]);
        assert_eq!(ids(&all(&idx, "#Home_Lab")), vec!["01A"]);
        assert_eq!(ids(&all(&idx, "#serie")), vec!["01B"]);
    }

    #[test]
    fn newest_first_by_date_then_time() {
        let idx = index(&[
            note("01A", "2026-09-21", "23:00", "x", None),
            note("01B", "2026-09-22", "08:00", "x", None),
            note("01C", "2026-09-22", "17:00", "x", None),
        ]);
        assert_eq!(ids(&all(&idx, "x")), vec!["01C", "01B", "01A"]);
    }

    #[test]
    fn containing_keeps_notes_with_any_needle_newest_first() {
        let idx = index(&[
            note("01A", "2026-09-21", "08:00", "- [ ] call the bank", None),
            note("01B", "2026-09-22", "08:00", "no tasks here", None),
            note("01C", "2026-09-22", "09:00", "Shopping\n- [X] milk", None),
        ]);
        let boxes = ["[ ]", "[x]", "[X]"].map(String::from);
        assert_eq!(ids(&containing(&idx, &boxes)), vec!["01C", "01A"]);
        assert_eq!(ids(&containing(&idx, &[String::new()])), vec!["01C", "01B", "01A"]);
        assert!(containing(&idx, &[]).is_empty());
    }

    #[test]
    fn results_carry_the_body_for_the_note_card() {
        let idx = index(&[note("01A", "2026-09-22", "08:00", "the body", None)]);
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
                note(&format!("{i:05}"), &date, "12:00", &body, Some("infra"))
            })
            .collect();
        let idx = index(&notes);

        let started = std::time::Instant::now();
        let hits = all(&idx, "#infra deploiement friday");
        let took = started.elapsed();

        assert_eq!(hits.len(), 10_000);
        eprintln!("searched 10,000 notes in {took:?}");
        if !cfg!(debug_assertions) {
            assert!(took.as_millis() < 50, "took {took:?}");
        }
    }
}
