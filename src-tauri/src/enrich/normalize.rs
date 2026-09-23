//! Tag normalisation, SPEC 5.5, applied to everything the model returns.
//!
//! The point is that the vocabulary stays small over time. A model left to
//! itself will happily invent `meetings` next to `meeting` and `argo-cd` next
//! to `argocd`, and after a few hundred notes the tag list is useless.

use std::collections::HashMap;

use strsim::levenshtein;
use unicode_normalization::UnicodeNormalization;

use super::grammar::{MAX_TAG, MAX_TAGS};

/// Below this length, an edit of one is far more likely to be a different tag
/// than a typo (`ci` and `cd`, `dev` and `ops`), so step 4 leaves them alone.
const FUZZY_MIN_LEN: usize = 5;

/// Words that name the kind of note rather than what it is about. They are
/// worthless for browsing, because every note is one of them, and once a few
/// are in the vocabulary the prompt's "reuse an existing tag" pull keeps
/// electing them for everything. The prompt already discourages these; this
/// makes it certain.
const GENRE_TAGS: [&str; 9] = [
    "issue", "plan", "update", "sync", "task", "note", "todo", "misc", "general",
];

/// What the tags are normalised against. Milestone 4 fills this from
/// `tags.json`; until then it is whatever the index already holds.
#[derive(Debug, Default)]
pub struct Vocabulary {
    pub counts: HashMap<String, u32>,
    pub aliases: HashMap<String, String>,
}

impl Vocabulary {
    pub fn knows(&self, tag: &str) -> bool {
        self.counts.contains_key(tag)
    }

    /// Existing tags, most used first, for the prompt's EXISTING TAGS list.
    pub fn by_frequency(&self) -> Vec<String> {
        let mut tags: Vec<(&String, &u32)> = self.counts.iter().collect();
        tags.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        tags.into_iter().map(|(tag, _)| tag.clone()).collect()
    }
}

/// Runs the five steps in order and returns at most `MAX_TAGS` tags.
pub fn normalize(raw: &[String], vocabulary: &Vocabulary) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    for tag in raw {
        let Some(tag) = clean(tag) else { continue };
        let tag = resolve_alias(tag, vocabulary);
        let tag = singularize(tag, vocabulary);
        let tag = merge_near_miss(tag, vocabulary);

        if !out.contains(&tag) {
            out.push(tag);
        }
        if out.len() == MAX_TAGS {
            break;
        }
    }

    drop_genre_tags(out)
}

/// Step 6. Remove tags that describe the kind of note, unless that would leave
/// the note with none: a bad tag still beats no tag at all.
fn drop_genre_tags(tags: Vec<String>) -> Vec<String> {
    let kept: Vec<String> = tags
        .iter()
        .filter(|tag| !GENRE_TAGS.contains(&tag.as_str()))
        .cloned()
        .collect();
    if kept.is_empty() {
        tags
    } else {
        kept
    }
}

/// Step 1. Lowercase, trim, strip a leading `#`, spaces and underscores become
/// hyphens, and anything that is not a letter, digit or hyphen goes. Letters
/// are Unicode, so `réunion` survives intact.
fn clean(raw: &str) -> Option<String> {
    let mut cleaned = String::with_capacity(raw.len());
    let mut last_was_hyphen = false;

    for ch in raw.trim().trim_start_matches('#').nfc() {
        let ch = ch.to_lowercase().next().unwrap_or(ch);
        let mapped = match ch {
            ' ' | '_' | '-' => '-',
            c if c.is_alphanumeric() => c,
            _ => continue,
        };
        // Collapse runs of hyphens rather than emitting `a--b`.
        if mapped == '-' {
            if last_was_hyphen || cleaned.is_empty() {
                continue;
            }
            last_was_hyphen = true;
        } else {
            last_was_hyphen = false;
        }
        cleaned.push(mapped);
    }

    while cleaned.ends_with('-') {
        cleaned.pop();
    }
    if cleaned.chars().count() > MAX_TAG {
        cleaned = cleaned.chars().take(MAX_TAG).collect();
        while cleaned.ends_with('-') {
            cleaned.pop();
        }
    }

    (!cleaned.is_empty()).then_some(cleaned)
}

/// Step 2.
fn resolve_alias(tag: String, vocabulary: &Vocabulary) -> String {
    vocabulary.aliases.get(&tag).cloned().unwrap_or(tag)
}

/// Step 3. Only when the singular is already a tag we use, so `kubernetes`
/// does not quietly become `kubernete`.
fn singularize(tag: String, vocabulary: &Vocabulary) -> String {
    if vocabulary.knows(&tag) {
        return tag;
    }
    for suffix in ["ies", "es", "s"] {
        let Some(stem) = tag.strip_suffix(suffix) else {
            continue;
        };
        let candidate = if suffix == "ies" {
            format!("{stem}y")
        } else {
            stem.to_string()
        };
        if !candidate.is_empty() && vocabulary.knows(&candidate) {
            return candidate;
        }
    }
    tag
}

/// Step 4. A new tag within one edit of an established one is almost always a
/// typo or a spelling variant, so fold it in.
fn merge_near_miss(tag: String, vocabulary: &Vocabulary) -> String {
    if vocabulary.knows(&tag) || tag.chars().count() < FUZZY_MIN_LEN {
        return tag;
    }
    vocabulary
        .counts
        .keys()
        .filter(|known| known.chars().count() >= FUZZY_MIN_LEN)
        .find(|known| levenshtein(known, &tag) == 1)
        .cloned()
        .unwrap_or(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocabulary(tags: &[(&str, u32)], aliases: &[(&str, &str)]) -> Vocabulary {
        Vocabulary {
            counts: tags.iter().map(|(t, c)| (t.to_string(), *c)).collect(),
            aliases: aliases
                .iter()
                .map(|(from, to)| (from.to_string(), to.to_string()))
                .collect(),
        }
    }

    fn normalized(raw: &[&str], vocabulary: &Vocabulary) -> Vec<String> {
        let raw: Vec<String> = raw.iter().map(|s| s.to_string()).collect();
        normalize(&raw, vocabulary)
    }

    #[test]
    fn lowercases_strips_hashes_and_hyphenates() {
        let vocabulary = Vocabulary::default();
        assert_eq!(
            normalized(&["#ArgoCD", "Deployment Plan", "some_tag"], &vocabulary),
            vec!["argocd", "deployment-plan", "some-tag"]
        );
    }

    #[test]
    fn drops_punctuation_but_keeps_accented_letters() {
        let vocabulary = Vocabulary::default();
        assert_eq!(
            normalized(&["réunion!", "c++", "a/b"], &vocabulary),
            vec!["réunion", "c", "ab"]
        );
    }

    #[test]
    fn collapses_hyphen_runs_and_trims_them() {
        let vocabulary = Vocabulary::default();
        assert_eq!(
            normalized(&["  --home  lab-- "], &vocabulary),
            vec!["home-lab"]
        );
    }

    #[test]
    fn discards_a_tag_that_cleans_away_to_nothing() {
        let vocabulary = Vocabulary::default();
        assert_eq!(normalized(&["###", "!!!", "ok"], &vocabulary), vec!["ok"]);
    }

    #[test]
    fn caps_a_tag_at_the_grammar_length() {
        let vocabulary = Vocabulary::default();
        let long = "a".repeat(80);
        let out = normalized(&[&long], &vocabulary);
        assert_eq!(out[0].chars().count(), MAX_TAG);
    }

    #[test]
    fn resolves_aliases() {
        let vocabulary = vocabulary(&[("kubernetes", 4)], &[("k8s", "kubernetes")]);
        assert_eq!(normalized(&["k8s"], &vocabulary), vec!["kubernetes"]);
    }

    #[test]
    fn singularizes_only_when_the_singular_is_already_known() {
        let known = vocabulary(&[("meeting", 9)], &[]);
        assert_eq!(normalized(&["meetings"], &known), vec!["meeting"]);

        // Nothing to fold into, so it is left alone.
        let empty = Vocabulary::default();
        assert_eq!(normalized(&["meetings"], &empty), vec!["meetings"]);
    }

    #[test]
    fn does_not_singularize_a_tag_it_already_knows() {
        let vocabulary = vocabulary(&[("kubernetes", 4)], &[]);
        assert_eq!(normalized(&["kubernetes"], &vocabulary), vec!["kubernetes"]);
    }

    #[test]
    fn folds_a_near_miss_into_the_established_tag() {
        let vocabulary = vocabulary(&[("staging", 12)], &[]);
        assert_eq!(normalized(&["stagging"], &vocabulary), vec!["staging"]);
    }

    #[test]
    fn leaves_short_tags_alone_even_one_edit_apart() {
        let vocabulary = vocabulary(&[("ci", 5), ("dev", 7)], &[]);
        assert_eq!(normalized(&["cd"], &vocabulary), vec!["cd"]);
        assert_eq!(normalized(&["ops"], &vocabulary), vec!["ops"]);
    }

    #[test]
    fn deduplicates_after_normalising() {
        let vocabulary = vocabulary(&[("meeting", 3)], &[("call", "meeting")]);
        assert_eq!(
            normalized(&["Meeting", "meetings", "call"], &vocabulary),
            vec!["meeting"]
        );
    }

    #[test]
    fn caps_the_number_of_tags() {
        let vocabulary = Vocabulary::default();
        let many = ["a1", "b2", "c3", "d4", "e5", "f6", "g7"];
        assert_eq!(normalized(&many, &vocabulary).len(), MAX_TAGS);
    }

    #[test]
    fn drops_tags_that_name_the_kind_of_note() {
        let vocabulary = Vocabulary::default();
        assert_eq!(
            normalized(&["issue", "grammar", "sync", "llama"], &vocabulary),
            vec!["grammar", "llama"]
        );
    }

    #[test]
    fn keeps_a_genre_tag_when_it_is_all_there_is() {
        let vocabulary = Vocabulary::default();
        // A poor tag still beats leaving the note untagged.
        assert_eq!(
            normalized(&["issue", "plan"], &vocabulary),
            vec!["issue", "plan"]
        );
    }

    #[test]
    fn a_genre_word_inside_a_longer_tag_is_left_alone() {
        let vocabulary = Vocabulary::default();
        assert_eq!(
            normalized(&["issue-tracker", "release-plan"], &vocabulary),
            vec!["issue-tracker", "release-plan"]
        );
    }

    #[test]
    fn orders_the_vocabulary_by_use() {
        let vocabulary = vocabulary(&[("rare", 1), ("common", 30), ("mid", 7)], &[]);
        assert_eq!(vocabulary.by_frequency(), vec!["common", "mid", "rare"]);
    }
}
