//! `categories.json`, the categories the model files notes under.
//!
//! The user's list: seeded on first run, edited by hand, and added to when
//! the model finds nothing in it that fits. It is read afresh for every note,
//! so an edit applies from the next one on.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::enrich::normalize;

/// What a new install starts with. Broad enough to cover most notes, narrow
/// enough that films, shows and games do not end up on one shelf.
pub const DEFAULT: [&str; 9] = [
    "development",
    "infrastructure",
    "movie",
    "tv",
    "game",
    "music",
    "book",
    "food",
    "home",
];

pub fn categories_path(root: &Path) -> PathBuf {
    root.join(".scratchnote").join("categories.json")
}

/// Write the default list if there is no file yet.
pub fn ensure(root: &Path) {
    let path = categories_path(root);
    if path.exists() {
        return;
    }
    let defaults: Vec<String> = DEFAULT.iter().map(|c| c.to_string()).collect();
    let written = std::fs::create_dir_all(root.join(".scratchnote"))
        .and_then(|_| std::fs::write(&path, render(&defaults)));
    if let Err(e) = written {
        log::warn!("could not write categories.json ({e})");
    }
}

/// The categories in file order. A missing or broken file offers the
/// defaults, so labelling still works.
pub fn load(root: &Path) -> Vec<String> {
    read(root).unwrap_or_else(|| DEFAULT.iter().map(|c| c.to_string()).collect())
}

/// The file with `category` appended, or `None` when there is nothing to
/// write: the category is already listed, empty, or the file is broken and
/// must not be overwritten.
pub fn with_added(root: &Path, category: &str) -> Option<String> {
    if category.is_empty() {
        return None;
    }
    let mut list = if categories_path(root).exists() {
        read(root)?
    } else {
        DEFAULT.iter().map(|c| c.to_string()).collect()
    };
    if list.iter().any(|c| c == category) {
        return None;
    }
    list.push(category.to_string());
    Some(render(&list))
}

/// The categories that at least one note carries, with how many do, most
/// used first. A category is a tag on the note, so the tag counts say it.
pub fn in_use(list: &[String], counts: &HashMap<String, u32>) -> Vec<(String, u32)> {
    let mut used: Vec<(String, u32)> = list
        .iter()
        .filter_map(|c| counts.get(c).map(|n| (c.clone(), *n)))
        .filter(|(_, n)| *n > 0)
        .collect();
    used.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    used
}

fn read(root: &Path) -> Option<Vec<String>> {
    let raw = std::fs::read_to_string(categories_path(root)).ok()?;
    match serde_json::from_str::<Vec<String>>(&raw) {
        Ok(raw) => {
            // Cleaned like tags, since that is what they become.
            let mut list: Vec<String> = Vec::new();
            for category in raw.iter().filter_map(|c| normalize::clean(c)) {
                if !list.contains(&category) {
                    list.push(category);
                }
            }
            Some(list)
        }
        Err(e) => {
            log::warn!("categories.json is not readable ({e}), using the defaults");
            None
        }
    }
}

fn render(list: &[String]) -> String {
    serde_json::to_string_pretty(list).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn a_new_install_gets_the_defaults_on_disk() {
        let root = root("scratchnote-categories-new");
        ensure(&root);
        assert_eq!(load(&root), DEFAULT.map(String::from).to_vec());
        assert!(load(&root).contains(&"movie".to_string()));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn keeps_the_users_list_cleaned_and_in_order() {
        let root = root("scratchnote-categories-edited");
        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
        std::fs::write(categories_path(&root), r#"["Movie", "board games", "movie"]"#).unwrap();
        ensure(&root);
        assert_eq!(load(&root), vec!["movie", "board-games"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn appends_only_a_category_it_does_not_have() {
        let root = root("scratchnote-categories-add");
        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
        std::fs::write(categories_path(&root), r#"["movie"]"#).unwrap();
        assert_eq!(with_added(&root, "movie"), None);
        assert_eq!(with_added(&root, ""), None);
        let added = with_added(&root, "podcast").unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&added).unwrap(),
            vec!["movie", "podcast"]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn lists_only_categories_in_use_most_used_first() {
        let list: Vec<String> = ["movie", "game", "book", "food"].map(String::from).to_vec();
        let counts: HashMap<String, u32> = [("game", 3), ("movie", 5), ("food", 3), ("argocd", 9)]
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect();
        assert_eq!(
            in_use(&list, &counts),
            vec![
                ("movie".to_string(), 5),
                ("food".to_string(), 3),
                ("game".to_string(), 3)
            ]
        );
    }

    #[test]
    fn never_overwrites_a_broken_file() {
        let root = root("scratchnote-categories-broken");
        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
        std::fs::write(categories_path(&root), "not json").unwrap();
        assert_eq!(load(&root), DEFAULT.map(String::from).to_vec());
        assert_eq!(with_added(&root, "podcast"), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
