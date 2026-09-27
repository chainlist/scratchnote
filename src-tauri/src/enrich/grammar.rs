//! The grammar that constrains enrichment output (SPEC 5.3).
//!
//! The grammar itself lives in `enrichment.gbnf` next to this file and is
//! embedded at compile time, so it stays a real grammar file that can be read,
//! edited and fed to llama.cpp tooling directly, with no runtime path to
//! resolve and nothing to ship alongside the binary. Only the category rule
//! is written here, from the list the note is labelled against.

const ENRICHMENT_GBNF: &str = include_str!("enrichment.gbnf");

/// Mirrors of the limits written into the grammar file. Used when validating
/// what came back; a test below fails if they drift apart.
pub const MAX_SUBJECT: usize = 60;
/// The longest category name, which `normalize::clean` cuts to.
pub const MAX_CATEGORY: usize = 30;

/// The grammar for a note filed under one of `categories`, or none: an empty
/// category is the model saying none of them is what the note is about.
/// Names come through `normalize::clean`, so none holds a quote or a
/// backslash that would need escaping in a literal.
pub fn enrichment(categories: &[String]) -> String {
    let names: Vec<String> = categories
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect();
    let category = if names.is_empty() {
        "category ::= \"\\\"\\\"\"".to_string()
    } else {
        format!("category ::= \"\\\"\" ({})? \"\\\"\"", names.join(" | "))
    };
    format!("{ENRICHMENT_GBNF}{category}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(grammar: &str) -> String {
        grammar
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Rule names defined in the grammar, whatever the alignment.
    fn defined(grammar: &str) -> Vec<String> {
        rules(grammar)
            .lines()
            .filter_map(|line| line.split_once("::="))
            .map(|(name, _)| name.trim().to_string())
            .collect()
    }

    fn rule(grammar: &str, name: &str) -> String {
        rules(grammar)
            .lines()
            .find(|line| line.starts_with(name))
            .unwrap_or_else(|| panic!("no {name} rule"))
            .to_string()
    }

    fn listed() -> String {
        enrichment(&["movie".to_string(), "série".to_string()])
    }

    #[test]
    fn states_every_rule_the_sampler_needs() {
        // A rule name that is referenced but never defined makes llama.cpp
        // reject the grammar at load time, which would fail every job.
        for grammar in [listed(), enrichment(&[])] {
            assert_eq!(
                defined(&grammar),
                ["root", "subject", "char", "ws", "category"],
                "{grammar}"
            );
        }
    }

    #[test]
    fn the_category_is_one_of_the_list_or_empty() {
        assert_eq!(
            rule(&listed(), "category"),
            r#"category ::= "\"" ("movie" | "série")? "\"""#
        );
        assert_eq!(rule(&enrichment(&[]), "category"), r#"category ::= "\"\"""#);
    }

    #[test]
    fn the_grammar_file_carries_the_same_limits_as_the_constants() {
        assert!(
            rules(ENRICHMENT_GBNF).contains(&format!("char{{1,{MAX_SUBJECT}}}")),
            "subject length disagrees with MAX_SUBJECT"
        );
    }

    #[test]
    fn pins_the_field_order_so_the_object_cannot_come_out_shuffled() {
        let root = rule(&listed(), "root");
        let subject = root.find("subject").unwrap();
        let category = root.find("category").unwrap();
        assert!(subject < category, "{root}");
    }

    #[test]
    fn whitespace_between_tokens_is_bounded() {
        let ws = rule(&listed(), "ws");
        // Unbounded, it spends output tokens on layout and can run the
        // answer into the token limit.
        assert!(!ws.contains('*') && !ws.contains('+'), "{ws}");
    }

    #[test]
    fn every_field_is_required() {
        let root = rule(&listed(), "root");
        // No optional markers anywhere in root: both fields must appear.
        assert!(!root.contains('?'), "a field is optional: {root}");
    }
}
