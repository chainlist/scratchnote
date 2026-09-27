/// What the model wrote, short enough for one log line. A failed parse is
/// only diagnosable with the output in hand.
pub fn excerpt(raw: &str) -> String {
    const LIMIT: usize = 300;
    let mut out: String = raw.chars().take(LIMIT).collect();
    if raw.chars().count() > LIMIT {
        out.push_str("...");
    }
    format!("{out:?}")
}

#[cfg(test)]
mod bench;
pub mod benchmark;
pub mod download;
pub mod grammar;
pub mod hardware;
pub mod idle;
pub mod language;
pub mod llama;
pub mod model;
pub mod normalize;
pub mod prompt;
pub mod queue;
pub mod runner;
pub mod worker;

use serde::{Deserialize, Serialize};

use grammar::MAX_SUBJECT;

/// What the model is asked for, and what gets written back into the markdown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Enrichment {
    pub subject: String,
    /// One of the categories offered, or empty when none of them fits.
    pub category: String,
}

impl Enrichment {
    /// The grammar already rules out malformed output, so this is a belt on
    /// top of braces: it also covers a backend that ignores the grammar, such
    /// as the stub used in tests.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let trimmed = raw.trim();
        let mut parsed: Enrichment = serde_json::from_str(trimmed).map_err(|e| {
            format!(
                "not the expected JSON ({e}), the model returned {}",
                excerpt(trimmed)
            )
        })?;

        parsed.subject = clamp(parsed.subject.trim(), MAX_SUBJECT);
        parsed.category = parsed.category.trim().to_string();

        if parsed.subject.is_empty() {
            return Err("the model returned an empty subject".to_string());
        }

        Ok(parsed)
    }
}

/// A subject is a heading, so it may not carry a newline into the markdown.
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
    fn a_parse_failure_quotes_what_the_model_returned() {
        let err = Enrichment::parse(r#"{"subject":"cut","category":"mo"#).unwrap_err();
        assert!(err.contains(r#"\"category\":\"mo"#), "{err}");
        let long = "x".repeat(1000);
        assert!(Enrichment::parse(&long).unwrap_err().ends_with("...\""));
    }

    #[test]
    fn parses_what_the_grammar_produces() {
        let out = Enrichment::parse(r#"{"subject":"Rollback plan","category":"infrastructure"}"#)
            .unwrap();
        assert_eq!(out.subject, "Rollback plan");
        assert_eq!(out.category, "infrastructure");
        let none = Enrichment::parse(r#"{"subject":"Hello","category":""}"#).unwrap();
        assert_eq!(none.category, "");
    }

    #[test]
    fn tolerates_surrounding_whitespace() {
        assert!(Enrichment::parse("\n  {\"subject\":\"a\",\"category\":\"b\"}  \n").is_ok());
    }

    #[test]
    fn rejects_output_that_is_not_the_object() {
        for bad in ["", "not json", r#"{"subject":"a"}"#, r#"{"category":"b"}"#] {
            assert!(Enrichment::parse(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn rejects_an_empty_subject() {
        assert!(Enrichment::parse(r#"{"subject":"  ","category":"b"}"#).is_err());
    }

    #[test]
    fn flattens_newlines_so_the_heading_stays_one_line() {
        let out = Enrichment::parse("{\"subject\":\"two\\nlines\",\"category\":\"b\"}").unwrap();
        assert_eq!(out.subject, "two lines");
    }

    #[test]
    fn clamps_the_subject_even_if_the_backend_ignored_the_grammar() {
        let long = "x".repeat(500);
        let raw = format!(r#"{{"subject":"{long}","category":"b"}}"#);
        let out = Enrichment::parse(&raw).unwrap();
        assert_eq!(out.subject.chars().count(), MAX_SUBJECT);
    }
}
