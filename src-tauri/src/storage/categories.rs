//! `categories.json`, the categories the model files notes under.
//!
//! One list of English names per space: seeded with the defaults, edited by
//! hand, and added to when a category is typed in the note editor. The model
//! only picks from it, whatever language notes are labelled in, and the
//! interface shows each name in its own language. It is read afresh for
//! every note, so an edit applies from the next one on.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::enrich::normalize;

/// What a space starts with. Broad enough to cover most notes, narrow enough
/// that films, shows and games do not end up on one shelf.
const DEFAULTS: [&str; 9] = [
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

fn defaults() -> Vec<String> {
    DEFAULTS.map(String::from).to_vec()
}

pub fn categories_path(root: &Path) -> PathBuf {
    root.join(".scratchnote").join("categories.json")
}

/// Write the defaults if there is no file yet, and turn a file from when
/// each language had its own list into the English list alone.
pub fn ensure(root: &Path) {
    let path = categories_path(root);
    let contents = match std::fs::read_to_string(&path) {
        Err(_) => render(&defaults()),
        Ok(raw) => match serde_json::from_str::<BTreeMap<String, Vec<String>>>(&raw) {
            Ok(mut lists) => render(&cleaned(lists.remove("en").unwrap_or_else(defaults))),
            Err(_) => return,
        },
    };
    let written = std::fs::create_dir_all(root.join(".scratchnote"))
        .and_then(|_| std::fs::write(&path, contents));
    if let Err(e) = written {
        log::warn!("could not write categories.json ({e})");
    }
}

/// The categories in file order. A missing or broken file offers the
/// defaults.
pub fn load(root: &Path) -> Vec<String> {
    read(root).unwrap_or_else(defaults)
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
        defaults()
    };
    if list.iter().any(|c| c == category) {
        return None;
    }
    list.push(category.to_string());
    Some(render(&list))
}

/// The categories that at least one note carries, with how many do, most
/// used first.
pub fn in_use(list: &[String], counts: &HashMap<String, u32>) -> Vec<(String, u32)> {
    let mut used: Vec<(String, u32)> = list
        .iter()
        .filter_map(|c| counts.get(c).map(|n| (c.clone(), *n)))
        .filter(|(_, n)| *n > 0)
        .collect();
    used.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    used
}

/// The list as written, cleaned. A file still holding one list per language
/// reads as its English one.
fn read(root: &Path) -> Option<Vec<String>> {
    let raw = std::fs::read_to_string(categories_path(root)).ok()?;
    let list = serde_json::from_str::<Vec<String>>(&raw).or_else(|_| {
        serde_json::from_str::<BTreeMap<String, Vec<String>>>(&raw)
            .map(|mut lists| lists.remove("en").unwrap_or_else(defaults))
    });
    match list {
        Ok(list) => Some(cleaned(list)),
        Err(e) => {
            log::warn!("categories.json is not readable ({e}), using the defaults");
            None
        }
    }
}

/// Cleaned as a category typed in the editor is, duplicates dropped.
fn cleaned(raw: Vec<String>) -> Vec<String> {
    let mut list: Vec<String> = Vec::new();
    for category in raw.iter().filter_map(|c| normalize::clean(c)) {
        if !list.contains(&category) {
            list.push(category);
        }
    }
    list
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

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn write(root: &Path, contents: &str) {
        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
        std::fs::write(categories_path(root), contents).unwrap();
    }

    #[test]
    fn a_new_space_gets_the_defaults_on_disk() {
        let root = root("scratchnote-categories-new");
        ensure(&root);
        assert_eq!(load(&root), defaults());
        let raw = std::fs::read_to_string(categories_path(&root)).unwrap();
        assert_eq!(serde_json::from_str::<Vec<String>>(&raw).unwrap(), defaults());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_list_per_language_becomes_the_english_one() {
        let root = root("scratchnote-categories-languages");
        write(
            &root,
            r#"{"en": ["movie", "Admin Dashboard"], "fr": ["film", "saison"]}"#,
        );
        assert_eq!(load(&root), strings(&["movie", "admin-dashboard"]));
        ensure(&root);
        let raw = std::fs::read_to_string(categories_path(&root)).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&raw).unwrap(),
            strings(&["movie", "admin-dashboard"])
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_list_with_no_english_one_starts_from_the_defaults() {
        let root = root("scratchnote-categories-french-only");
        write(&root, r#"{"fr": ["film"]}"#);
        assert_eq!(load(&root), defaults());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn keeps_the_users_list_cleaned_and_in_order() {
        let root = root("scratchnote-categories-edited");
        write(&root, r#"["Movie", "board games", "movie"]"#);
        ensure(&root);
        assert_eq!(load(&root), strings(&["movie", "board-games"]));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn appends_only_a_category_it_does_not_have() {
        let root = root("scratchnote-categories-add");
        write(&root, r#"["movie"]"#);
        assert_eq!(with_added(&root, "movie"), None);
        assert_eq!(with_added(&root, ""), None);
        let added = with_added(&root, "podcast").unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&added).unwrap(),
            strings(&["movie", "podcast"])
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn lists_only_categories_in_use_most_used_first() {
        let list = strings(&["movie", "game", "book", "food"]);
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
        write(&root, "not json");
        ensure(&root);
        assert_eq!(
            std::fs::read_to_string(categories_path(&root)).unwrap(),
            "not json"
        );
        assert_eq!(load(&root), defaults());
        assert_eq!(with_added(&root, "podcast"), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
