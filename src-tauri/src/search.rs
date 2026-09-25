//! In-memory search, SPEC 6.
//!
//! A query is words and `#tag` tokens. Every word must appear somewhere in the
//! subject, summary or body, and every tag must be on the note. Matching
//! ignores case and accents, so `reunion` finds `Réunion`.

use std::collections::HashMap;

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use crate::enrich::normalize;
use crate::storage::daily_file::Note;
use crate::storage::index::Index;

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
    tags: Vec<String>,
}

impl Query {
    /// Tag tokens go through the same cleaning and aliases as the model's
    /// tags, so `#K8s` finds notes tagged `kubernetes` when that alias exists.
    fn parse(raw: &str, aliases: &HashMap<String, String>) -> Self {
        let mut query = Query::default();
        for token in raw.split_whitespace() {
            if token.starts_with('#') {
                if let Some(tag) = normalize::clean(token) {
                    let tag = aliases.get(&tag).cloned().unwrap_or(tag);
                    query.tags.push(fold(&tag));
                }
            } else {
                query.words.push(fold(token));
            }
        }
        query
    }

    fn is_empty(&self) -> bool {
        self.words.is_empty() && self.tags.is_empty()
    }
}

/// Newest first: by date, then by time within the day.
pub fn search(index: &Index, raw: &str, aliases: &HashMap<String, String>) -> Vec<Note> {
    let query = Query::parse(raw, aliases);
    if query.is_empty() {
        return Vec::new();
    }

    let mut hits: Vec<Note> = index
        .entries()
        .filter(|entry| {
            query
                .tags
                .iter()
                .all(|wanted| entry.tags.iter().any(|tag| fold(tag) == *wanted))
                && query.words.iter().all(|word| entry.folded.contains(word))
        })
        .map(|entry| entry.to_note())
        .collect();

    hits.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.time.cmp(&a.time)));
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::daily_file::{body_hash, Status};
    use crate::storage::index::IndexEntry;
    use crate::storage::relative_day_path;

    fn note(id: &str, date: &str, time: &str, body: &str, tags: &[&str]) -> Note {
        Note {
            id: id.to_string(),
            date: date.to_string(),
            time: time.to_string(),
            file: relative_day_path(date),
            subject: None,
            summary: None,
            tags: tags.iter().map(|t| t.to_string()).collect(),
            status: Status::Done,
            hash: body_hash(body),
            lang: None,
            body: body.to_string(),
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

    fn no_aliases() -> HashMap<String, String> {
        HashMap::new()
    }

    #[test]
    fn folds_case_and_accents() {
        assert_eq!(fold("Réunion ÉQUIPE"), "reunion equipe");
    }

    #[test]
    fn splits_words_from_tags() {
        let query = Query::parse("#Infra  kubernetes #k8s", &no_aliases());
        assert_eq!(query.words, vec!["kubernetes"]);
        assert_eq!(query.tags, vec!["infra", "k8s"]);
    }

    #[test]
    fn an_empty_query_finds_nothing() {
        let idx = index(&[note("01A", "2026-09-22", "08:00", "anything", &[])]);
        assert!(search(&idx, "   ", &no_aliases()).is_empty());
        assert!(search(&idx, "#", &no_aliases()).is_empty());
    }

    #[test]
    fn matches_substrings_ignoring_case_and_accents() {
        let idx = index(&[
            note("01A", "2026-09-22", "08:00", "Réunion demain", &[]),
            note("01B", "2026-09-22", "09:00", "Buy coffee", &[]),
        ]);
        assert_eq!(ids(&search(&idx, "REUNION", &no_aliases())), vec!["01A"]);
        assert_eq!(ids(&search(&idx, "offe", &no_aliases())), vec!["01B"]);
    }

    #[test]
    fn searches_subject_and_summary_too() {
        let mut n = note("01A", "2026-09-22", "08:00", "body text", &[]);
        n.subject = Some("Rollback plan".into());
        n.summary = Some("Pin the chart".into());
        let idx = index(&[n]);
        assert_eq!(ids(&search(&idx, "rollback", &no_aliases())), vec!["01A"]);
        assert_eq!(ids(&search(&idx, "chart", &no_aliases())), vec!["01A"]);
    }

    #[test]
    fn every_word_must_match() {
        let idx = index(&[
            note("01A", "2026-09-22", "08:00", "staging broke again", &[]),
            note("01B", "2026-09-22", "09:00", "staging is fine", &[]),
        ]);
        assert_eq!(
            ids(&search(&idx, "staging broke", &no_aliases())),
            vec!["01A"]
        );
    }

    #[test]
    fn tags_are_and_filters() {
        let idx = index(&[
            note("01A", "2026-09-22", "08:00", "a", &["infra", "kubernetes"]),
            note("01B", "2026-09-22", "09:00", "b", &["infra"]),
            note("01C", "2026-09-22", "10:00", "c", &["kubernetes"]),
        ]);
        assert_eq!(
            ids(&search(&idx, "#infra", &no_aliases())),
            vec!["01B", "01A"]
        );
        assert_eq!(
            ids(&search(&idx, "#infra #kubernetes", &no_aliases())),
            vec!["01A"]
        );
    }

    #[test]
    fn a_tag_must_match_whole_not_as_a_substring() {
        let idx = index(&[note("01A", "2026-09-22", "08:00", "a", &["infrastructure"])]);
        assert!(search(&idx, "#infra", &no_aliases()).is_empty());
    }

    #[test]
    fn combines_tags_and_words() {
        let idx = index(&[
            note(
                "01A",
                "2026-09-22",
                "08:00",
                "kubernetes upgrade",
                &["infra"],
            ),
            note("01B", "2026-09-22", "09:00", "kubernetes talk", &["meetup"]),
        ]);
        assert_eq!(
            ids(&search(&idx, "#infra kubernetes", &no_aliases())),
            vec!["01A"]
        );
    }

    #[test]
    fn tag_tokens_resolve_through_aliases_and_ignore_accents() {
        let idx = index(&[
            note("01A", "2026-09-22", "08:00", "a", &["kubernetes"]),
            note("01B", "2026-09-22", "09:00", "b", &["réunion"]),
        ]);
        let aliases: HashMap<String, String> =
            [("k8s".to_string(), "kubernetes".to_string())].into();
        assert_eq!(ids(&search(&idx, "#K8s", &aliases)), vec!["01A"]);
        assert_eq!(ids(&search(&idx, "#reunion", &aliases)), vec!["01B"]);
    }

    #[test]
    fn newest_first_by_date_then_time() {
        let idx = index(&[
            note("01A", "2026-09-21", "23:00", "x", &[]),
            note("01B", "2026-09-22", "08:00", "x", &[]),
            note("01C", "2026-09-22", "17:00", "x", &[]),
        ]);
        assert_eq!(
            ids(&search(&idx, "x", &no_aliases())),
            vec!["01C", "01B", "01A"]
        );
    }

    #[test]
    fn results_carry_the_body_for_the_note_card() {
        let idx = index(&[note("01A", "2026-09-22", "08:00", "the body", &[])]);
        assert_eq!(search(&idx, "body", &no_aliases())[0].body, "the body");
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
                note(
                    &format!("{i:05}"),
                    &date,
                    "12:00",
                    &body,
                    &["infra", "staging"],
                )
            })
            .collect();
        let idx = index(&notes);

        let started = std::time::Instant::now();
        let hits = search(&idx, "#infra deploiement friday", &no_aliases());
        let took = started.elapsed();

        assert_eq!(hits.len(), 10_000);
        eprintln!("searched 10,000 notes in {took:?}");
        if !cfg!(debug_assertions) {
            assert!(took.as_millis() < 50, "took {took:?}");
        }
    }
}
