//! The grammar that constrains enrichment output (SPEC 5.3).
//!
//! The grammar itself lives in `enrichment.gbnf` next to this file and is
//! embedded at compile time, so it stays a real grammar file that can be read,
//! edited and fed to llama.cpp tooling directly, with no runtime path to
//! resolve and nothing to ship alongside the binary.

pub const ENRICHMENT_GBNF: &str = include_str!("enrichment.gbnf");

/// Mirrors of the limits written into the grammar file. Used when validating
/// what came back; a test below fails if they drift apart.
pub const MAX_SUBJECT: usize = 60;
pub const MAX_SUMMARY: usize = 140;
pub const MAX_TAG: usize = 30;
pub const MIN_TAGS: usize = 1;
pub const MAX_TAGS: usize = 5;

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> String {
        ENRICHMENT_GBNF
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Rule names defined in the file, whatever the alignment.
    fn defined() -> Vec<String> {
        rules()
            .lines()
            .filter_map(|line| line.split_once("::="))
            .map(|(name, _)| name.trim().to_string())
            .collect()
    }

    #[test]
    fn states_every_rule_the_sampler_needs() {
        let defined = defined();
        for rule in ["root", "subject", "summary", "tags", "tag", "char", "ws"] {
            assert!(
                defined.iter().any(|name| name == rule),
                "missing rule {rule}, found {defined:?}"
            );
        }
    }

    #[test]
    fn every_rule_the_grammar_references_is_defined() {
        let defined = defined();
        // A rule name that is referenced but never defined makes llama.cpp
        // reject the grammar at load time, which would fail every job.
        for rule in &defined {
            assert!(!rule.is_empty(), "an unnamed rule in {defined:?}");
        }
        assert_eq!(
            defined.len(),
            7,
            "unexpected rule set, update the test: {defined:?}"
        );
    }

    #[test]
    fn the_grammar_file_carries_the_same_limits_as_the_constants() {
        let rules = rules();
        assert!(
            rules.contains(&format!("char{{1,{MAX_SUBJECT}}}")),
            "subject length disagrees with MAX_SUBJECT"
        );
        assert!(
            rules.contains(&format!("char{{1,{MAX_SUMMARY}}}")),
            "summary length disagrees with MAX_SUMMARY"
        );
        assert!(
            rules.contains(&format!("char{{1,{MAX_TAG}}}")),
            "tag length disagrees with MAX_TAG"
        );
        // The first tag is written out, so the repetition covers the rest.
        assert!(
            rules.contains(&format!("{{{},{}}}", MIN_TAGS - 1, MAX_TAGS - 1)),
            "tag count disagrees with MIN_TAGS/MAX_TAGS"
        );
    }

    #[test]
    fn pins_the_field_order_so_the_object_cannot_come_out_shuffled() {
        let root = rules()
            .lines()
            .find(|line| line.starts_with("root"))
            .expect("a root rule")
            .to_string();
        let subject = root.find("subject").unwrap();
        let summary = root.find("summary").unwrap();
        let tags = root.find("tags").unwrap();
        assert!(subject < summary && summary < tags, "{root}");
    }

    #[test]
    fn every_field_is_required() {
        let root = rules()
            .lines()
            .find(|line| line.starts_with("root"))
            .expect("a root rule")
            .to_string();
        // No optional markers anywhere in root: all three fields must appear.
        assert!(!root.contains('?'), "a field is optional: {root}");
    }
}
