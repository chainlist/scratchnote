//! How a category is spelled, whether typed by hand, read from
//! `categories.json` or typed after `#` in a search.

use unicode_normalization::UnicodeNormalization;

use super::grammar::MAX_CATEGORY;

/// Lowercase, trim, strip a leading `#`, spaces and underscores become
/// hyphens, and anything that is not a letter, digit or hyphen goes. Letters
/// are Unicode, so `réunion` survives intact.
pub(crate) fn clean(raw: &str) -> Option<String> {
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
    if cleaned.chars().count() > MAX_CATEGORY {
        cleaned = cleaned.chars().take(MAX_CATEGORY).collect();
        while cleaned.ends_with('-') {
            cleaned.pop();
        }
    }

    (!cleaned.is_empty()).then_some(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cleaned(raw: &[&str]) -> Vec<String> {
        raw.iter().filter_map(|r| clean(r)).collect()
    }

    #[test]
    fn lowercases_strips_hashes_and_hyphenates() {
        assert_eq!(
            cleaned(&["#ArgoCD", "Deployment Plan", "some_tag"]),
            vec!["argocd", "deployment-plan", "some-tag"]
        );
    }

    #[test]
    fn drops_punctuation_but_keeps_accented_letters() {
        assert_eq!(
            cleaned(&["réunion!", "c++", "a/b"]),
            vec!["réunion", "c", "ab"]
        );
    }

    #[test]
    fn collapses_hyphen_runs_and_trims_them() {
        assert_eq!(cleaned(&["  --home  lab-- "]), vec!["home-lab"]);
    }

    #[test]
    fn a_name_that_cleans_away_to_nothing_is_none() {
        assert_eq!(cleaned(&["###", "!!!", "ok"]), vec!["ok"]);
    }

    #[test]
    fn caps_a_name_at_the_grammar_length() {
        let long = "a".repeat(80);
        assert_eq!(clean(&long).unwrap().chars().count(), MAX_CATEGORY);
    }
}
