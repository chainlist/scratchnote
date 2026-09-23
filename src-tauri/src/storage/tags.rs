//! `tags.json`, the tag vocabulary from SPEC 4.5.
//!
//! Two halves with different owners. The counts are derived from the index
//! and rewritten whenever it is; the aliases belong to the user and are only
//! ever read back and carried through, never computed.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct TagFile {
    tags: BTreeMap<String, u32>,
    aliases: BTreeMap<String, String>,
}

pub fn tags_path(root: &Path) -> PathBuf {
    root.join(".scratchnote").join("tags.json")
}

/// The user's aliases. A missing or unreadable file means none, which is the
/// state every install starts in.
pub fn load_aliases(root: &Path) -> HashMap<String, String> {
    let Ok(raw) = std::fs::read_to_string(tags_path(root)) else {
        return HashMap::new();
    };
    match serde_json::from_str::<TagFile>(&raw) {
        Ok(file) => file.aliases.into_iter().collect(),
        Err(e) => {
            log::warn!("tags.json is not readable ({e}), ignoring its aliases");
            HashMap::new()
        }
    }
}

/// Sorted keys, so the file diffs cleanly for anyone syncing it with git.
pub fn render(counts: &HashMap<String, u32>, aliases: &HashMap<String, String>) -> String {
    let file = TagFile {
        tags: counts.iter().map(|(t, c)| (t.clone(), *c)).collect(),
        aliases: aliases
            .iter()
            .map(|(from, to)| (from.clone(), to.clone()))
            .collect(),
    };
    serde_json::to_string_pretty(&file).unwrap_or_else(|_| "{}".to_string())
}

/// Most used first, then by name, which is the sidebar order (SPEC 3.2).
pub fn by_count(counts: &HashMap<String, u32>) -> Vec<(String, u32)> {
    let mut tags: Vec<(String, u32)> = counts.iter().map(|(t, c)| (t.clone(), *c)).collect();
    tags.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, u32)]) -> HashMap<String, u32> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn aliases(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn renders_the_shape_from_the_spec() {
        let out = render(&map(&[("argocd", 12)]), &aliases(&[("k8s", "kubernetes")]));
        let value: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(value["tags"]["argocd"], 12);
        assert_eq!(value["aliases"]["k8s"], "kubernetes");
    }

    #[test]
    fn aliases_survive_a_rewrite_of_the_counts() {
        let root = std::env::temp_dir().join("scratchnote-tags-roundtrip");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();

        let aliases = aliases(&[("meetings", "meeting")]);
        std::fs::write(tags_path(&root), render(&map(&[("meeting", 3)]), &aliases)).unwrap();
        assert_eq!(load_aliases(&root), aliases);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_or_broken_file_means_no_aliases() {
        let root = std::env::temp_dir().join("scratchnote-tags-broken");
        let _ = std::fs::remove_dir_all(&root);
        assert!(load_aliases(&root).is_empty());

        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
        std::fs::write(tags_path(&root), "not json").unwrap();
        assert!(load_aliases(&root).is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_with_only_aliases_is_fine() {
        let root = std::env::temp_dir().join("scratchnote-tags-partial");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
        std::fs::write(tags_path(&root), r#"{"aliases":{"k8s":"kubernetes"}}"#).unwrap();
        assert_eq!(load_aliases(&root)["k8s"], "kubernetes");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sorts_by_count_then_name() {
        let sorted = by_count(&map(&[("b", 2), ("a", 2), ("c", 9)]));
        assert_eq!(
            sorted,
            vec![("c".into(), 9), ("a".into(), 2), ("b".into(), 2)]
        );
    }
}
