#[cfg(test)]
mod bench;
pub mod download;
pub mod grammar;
pub mod idle;
pub mod llama;
pub mod model;
pub mod normalize;
pub mod prompt;
pub mod queue;
pub mod runner;
pub mod worker;

use serde::{Deserialize, Serialize};

use grammar::{MAX_SUBJECT, MAX_SUMMARY, MAX_TAGS, MIN_TAGS};

/// What the model is asked for, and what gets written back into the markdown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Enrichment {
    pub subject: String,
    pub summary: String,
    pub tags: Vec<String>,
}

impl Enrichment {
    /// The grammar already rules out malformed output, so this is a belt on
    /// top of braces: it also covers a backend that ignores the grammar, such
    /// as the stub used in tests.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let trimmed = raw.trim();
        let mut parsed: Enrichment =
            serde_json::from_str(trimmed).map_err(|e| format!("not the expected JSON: {e}"))?;

        parsed.subject = clamp(parsed.subject.trim(), MAX_SUBJECT);
        parsed.summary = clamp(parsed.summary.trim(), MAX_SUMMARY);

        if parsed.subject.is_empty() {
            return Err("the model returned an empty subject".to_string());
        }
        if parsed.tags.len() < MIN_TAGS {
            return Err("the model returned no tags".to_string());
        }
        parsed.tags.truncate(MAX_TAGS);

        Ok(parsed)
    }
}

/// A subject is a heading and a summary is a blockquote line, so neither may
/// carry a newline into the markdown.
fn clamp(value: &str, max: usize) -> String {
    let single_line: String = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    single_line
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max)
        .collect::<String>()
        .trim_end()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_what_the_grammar_produces() {
        let out = Enrichment::parse(
            r#"{"subject":"Rollback plan","summary":"Pin the chart.","tags":["argocd","staging"]}"#,
        )
        .unwrap();
        assert_eq!(out.subject, "Rollback plan");
        assert_eq!(out.summary, "Pin the chart.");
        assert_eq!(out.tags, vec!["argocd", "staging"]);
    }

    #[test]
    fn tolerates_surrounding_whitespace() {
        assert!(Enrichment::parse(
            "\n  {\"subject\":\"a\",\"summary\":\"b\",\"tags\":[\"c\"]}  \n"
        )
        .is_ok());
    }

    #[test]
    fn rejects_output_that_is_not_the_object() {
        for bad in [
            "",
            "not json",
            r#"{"subject":"a"}"#,
            r#"{"subject":"a","summary":"b"}"#,
        ] {
            assert!(Enrichment::parse(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn rejects_an_empty_subject_or_no_tags() {
        assert!(Enrichment::parse(r#"{"subject":"  ","summary":"b","tags":["c"]}"#).is_err());
        assert!(Enrichment::parse(r#"{"subject":"a","summary":"b","tags":[]}"#).is_err());
    }

    #[test]
    fn flattens_newlines_so_the_heading_stays_one_line() {
        let out = Enrichment::parse(
            "{\"subject\":\"two\\nlines\",\"summary\":\"a\\nb\",\"tags\":[\"c\"]}",
        )
        .unwrap();
        assert_eq!(out.subject, "two lines");
        assert_eq!(out.summary, "a b");
    }

    #[test]
    fn clamps_lengths_even_if_the_backend_ignored_the_grammar() {
        let long = "x".repeat(500);
        let raw = format!(r#"{{"subject":"{long}","summary":"{long}","tags":["a"]}}"#);
        let out = Enrichment::parse(&raw).unwrap();
        assert_eq!(out.subject.chars().count(), MAX_SUBJECT);
        assert_eq!(out.summary.chars().count(), MAX_SUMMARY);
    }

    #[test]
    fn caps_the_tag_count() {
        let raw = r#"{"subject":"a","summary":"b","tags":["1","2","3","4","5","6","7"]}"#;
        assert_eq!(Enrichment::parse(raw).unwrap().tags.len(), MAX_TAGS);
    }
}
