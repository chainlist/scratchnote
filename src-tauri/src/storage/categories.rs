//! `categories.json`, the categories the model files notes under.
//!
//! The user's lists, one per language notes are labelled in: seeded on first
//! use of a language, edited by hand, and added to when the model finds
//! nothing in one that fits. It is read afresh for every note, so an edit
//! applies from the next one on.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::enrich::language;
use crate::enrich::normalize;

/// What a language starts with. Broad enough to cover most notes, narrow
/// enough that films, shows and games do not end up on one shelf.
fn defaults(lang: &str) -> Vec<String> {
    let list: [&str; 9] = match lang {
        "fr" => [
            "développement",
            "infrastructure",
            "film",
            "série",
            "jeu",
            "musique",
            "livre",
            "cuisine",
            "maison",
        ],
        "es" => [
            "desarrollo",
            "infraestructura",
            "película",
            "serie",
            "juego",
            "música",
            "libro",
            "comida",
            "hogar",
        ],
        "de" => [
            "entwicklung",
            "infrastruktur",
            "film",
            "serie",
            "spiel",
            "musik",
            "buch",
            "essen",
            "zuhause",
        ],
        "it" => [
            "sviluppo",
            "infrastruttura",
            "film",
            "serie",
            "gioco",
            "musica",
            "libro",
            "cibo",
            "casa",
        ],
        "pt" => [
            "desenvolvimento",
            "infraestrutura",
            "filme",
            "série",
            "jogo",
            "música",
            "livro",
            "comida",
            "casa",
        ],
        _ => [
            "development",
            "infrastructure",
            "movie",
            "tv",
            "game",
            "music",
            "book",
            "food",
            "home",
        ],
    };
    list.map(String::from).to_vec()
}

/// Each language's list, by locale code.
type Lists = BTreeMap<String, Vec<String>>;

pub fn categories_path(root: &Path) -> PathBuf {
    root.join(".scratchnote").join("categories.json")
}

/// Write the defaults of `lang` if there is no file yet.
pub fn ensure(root: &Path, lang: &str) {
    let path = categories_path(root);
    if path.exists() {
        return;
    }
    let lists = Lists::from([(lang.to_string(), defaults(lang))]);
    let written = std::fs::create_dir_all(root.join(".scratchnote"))
        .and_then(|_| std::fs::write(&path, render(&lists)));
    if let Err(e) = written {
        log::warn!("could not write categories.json ({e})");
    }
}

/// Every language's categories, for telling a note's category from its tags
/// and for the sidebar and editor: a note labelled in another language keeps
/// its category. A missing or broken file offers the English defaults.
pub fn load(root: &Path) -> Vec<String> {
    let Some(lists) = read(root) else {
        return defaults(language::ENGLISH.code);
    };
    let mut all: Vec<String> = Vec::new();
    for category in lists.into_values().flatten() {
        if !all.contains(&category) {
            all.push(category);
        }
    }
    all
}

/// The categories of `lang` in file order, for the model to pick from. A
/// language with no list yet, or a broken file, offers its defaults.
pub fn load_in(root: &Path, lang: &str) -> Vec<String> {
    read(root)
        .and_then(|mut lists| lists.remove(lang))
        .unwrap_or_else(|| defaults(lang))
}

/// The file with `category` appended to the list of `lang`, or `None` when
/// there is nothing to write: the category is already listed, empty, or the
/// file is broken and must not be overwritten. A language with no list yet
/// gets its defaults written too, so the sidebar knows them.
pub fn with_added(root: &Path, lang: &str, category: &str) -> Option<String> {
    if category.is_empty() {
        return None;
    }
    let mut lists = if categories_path(root).exists() {
        read(root)?
    } else {
        Lists::new()
    };
    let listed = lists.contains_key(lang);
    let list = lists
        .entry(lang.to_string())
        .or_insert_with(|| defaults(lang));
    if list.iter().any(|c| c == category) {
        if listed {
            return None;
        }
    } else {
        list.push(category.to_string());
    }
    Some(render(&lists))
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

/// A note's category and the rest of its tags. The category is the first
/// tag, and only when it is on the list: enrichment always puts it there,
/// and a note whose first tag is anything else has none.
pub fn split(list: &[String], tags: &[String]) -> (Option<String>, Vec<String>) {
    match tags.split_first() {
        Some((first, rest)) if list.contains(first) => (Some(first.clone()), rest.to_vec()),
        _ => (None, tags.to_vec()),
    }
}

/// The reverse of `split`: the category first, then the other tags, with the
/// category dropped from them should it also be there.
pub fn join(category: Option<&str>, tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = category.map(str::to_string).into_iter().collect();
    for tag in tags {
        if !out.contains(tag) {
            out.push(tag.clone());
        }
    }
    out
}

/// The lists as written. A plain list, the format before languages had
/// their own, is the English one.
fn read(root: &Path) -> Option<Lists> {
    let raw = std::fs::read_to_string(categories_path(root)).ok()?;
    let lists = serde_json::from_str::<Lists>(&raw).or_else(|_| {
        serde_json::from_str::<Vec<String>>(&raw)
            .map(|list| Lists::from([(language::ENGLISH.code.to_string(), list)]))
    });
    match lists {
        Ok(lists) => Some(
            lists
                .into_iter()
                .map(|(lang, raw)| {
                    // Cleaned like tags, since that is what they become.
                    let mut list: Vec<String> = Vec::new();
                    for category in raw.iter().filter_map(|c| normalize::clean(c)) {
                        if !list.contains(&category) {
                            list.push(category);
                        }
                    }
                    (lang, list)
                })
                .collect(),
        ),
        Err(e) => {
            log::warn!("categories.json is not readable ({e}), using the defaults");
            None
        }
    }
}

fn render(lists: &Lists) -> String {
    serde_json::to_string_pretty(lists).unwrap_or_else(|_| "{}".to_string())
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
    fn a_new_install_gets_the_defaults_of_its_language_on_disk() {
        let root = root("scratchnote-categories-new");
        ensure(&root, "fr");
        assert_eq!(load_in(&root, "fr"), defaults("fr"));
        assert!(load(&root).contains(&"film".to_string()));
        assert!(!load(&root).contains(&"movie".to_string()));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_plain_list_is_the_english_one() {
        let root = root("scratchnote-categories-legacy");
        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
        std::fs::write(categories_path(&root), r#"["movie", "admin-dashboard"]"#).unwrap();
        assert_eq!(load_in(&root, "en"), vec!["movie", "admin-dashboard"]);
        // French has no list yet, so the model is offered its defaults.
        assert_eq!(load_in(&root, "fr"), defaults("fr"));
        assert_eq!(load(&root), vec!["movie", "admin-dashboard"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn every_language_is_listed_for_the_sidebar_once() {
        let root = root("scratchnote-categories-all");
        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
        std::fs::write(
            categories_path(&root),
            r#"{"en": ["movie", "infrastructure"], "fr": ["film", "infrastructure"]}"#,
        )
        .unwrap();
        assert_eq!(load(&root), vec!["movie", "infrastructure", "film"]);
        assert_eq!(load_in(&root, "fr"), vec!["film", "infrastructure"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn keeps_the_users_list_cleaned_and_in_order() {
        let root = root("scratchnote-categories-edited");
        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
        std::fs::write(
            categories_path(&root),
            r#"["Movie", "board games", "movie"]"#,
        )
        .unwrap();
        ensure(&root, "en");
        assert_eq!(load(&root), vec!["movie", "board-games"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn appends_only_a_category_it_does_not_have() {
        let root = root("scratchnote-categories-add");
        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
        std::fs::write(categories_path(&root), r#"["movie"]"#).unwrap();
        assert_eq!(with_added(&root, "en", "movie"), None);
        assert_eq!(with_added(&root, "en", ""), None);
        let added = with_added(&root, "en", "podcast").unwrap();
        assert_eq!(
            serde_json::from_str::<Lists>(&added).unwrap(),
            Lists::from([("en".to_string(), strings(&["movie", "podcast"]))])
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_first_category_of_a_language_writes_its_defaults_too() {
        let root = root("scratchnote-categories-new-language");
        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
        std::fs::write(categories_path(&root), r#"["movie"]"#).unwrap();
        // One of the defaults: nothing new to add, but the list gets written.
        let added = with_added(&root, "fr", "film").unwrap();
        let lists = serde_json::from_str::<Lists>(&added).unwrap();
        assert_eq!(lists["en"], strings(&["movie"]));
        assert_eq!(lists["fr"], defaults("fr"));
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

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_category_is_the_first_tag_only_when_it_is_listed() {
        let list = strings(&["movie", "game"]);
        assert_eq!(
            split(&list, &strings(&["movie", "dune", "scifi"])),
            (Some("movie".to_string()), strings(&["dune", "scifi"]))
        );
        assert_eq!(
            split(&list, &strings(&["dune", "movie"])),
            (None, strings(&["dune", "movie"]))
        );
        assert_eq!(split(&list, &[]), (None, Vec::new()));
    }

    #[test]
    fn joining_puts_the_category_first_once() {
        assert_eq!(
            join(Some("game"), &strings(&["silksong", "game"])),
            strings(&["game", "silksong"])
        );
        assert_eq!(join(None, &strings(&["a", "b"])), strings(&["a", "b"]));
    }

    #[test]
    fn never_overwrites_a_broken_file() {
        let root = root("scratchnote-categories-broken");
        std::fs::create_dir_all(root.join(".scratchnote")).unwrap();
        std::fs::write(categories_path(&root), "not json").unwrap();
        assert_eq!(load(&root), defaults("en"));
        assert_eq!(load_in(&root, "fr"), defaults("fr"));
        assert_eq!(with_added(&root, "en", "podcast"), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
