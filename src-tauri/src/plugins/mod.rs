//! Plugins, SPEC 3.9 and 4.9. The webview runs them; this side keeps what
//! is installed and switched on, hands out their code and data, and fetches
//! new ones from the registry.
//!
//! Everything lives in the notes root's `.scratchnote/`, as Obsidian keeps
//! plugins in the vault, so a synced root carries them along:
//!
//! - `plugins.json`: community plugins on or off, which are enabled, and
//!   which core plugins are switched off, or on for those that start off.
//! - `plugins/<id>/`: an installed community plugin, `manifest.json`,
//!   `main.js`, `styles.css` if it has one, and its `data.json`.
//! - `core-plugins/<id>.json`: a core plugin's data. Core plugins are built
//!   into the app, so there is no code to keep.
//!
//! The commands the settings and the webview call are in
//! `commands::plugins`.

pub mod registry;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::state::AppState;

const STATE_FILE: &str = "plugins.json";
const PLUGINS_DIR: &str = "plugins";
const CORE_DATA_DIR: &str = "core-plugins";
const DATA_FILE: &str = "data.json";

/// What `plugins.json` holds. Community plugins start off (SPEC 3.9): until
/// the user turns them on, none of their code is handed out.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PluginState {
    pub community: bool,
    /// Community plugins switched on, by id.
    pub enabled: Vec<String>,
    /// Core plugins switched off, by id. They are on unless listed here.
    pub core_disabled: Vec<String>,
    /// Core plugins that start off, switched on, by id.
    pub core_enabled: Vec<String>,
}

/// A plugin's `manifest.json`, as Obsidian's.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub min_app_version: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub author_url: Option<String>,
}

/// The state and every installed community plugin, by name.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginsView {
    #[serde(flatten)]
    pub state: PluginState,
    pub installed: Vec<Manifest>,
}

/// An installed plugin's code, for the webview to run.
#[derive(Debug, Serialize)]
pub struct PluginCode {
    pub main: String,
    pub styles: Option<String>,
}

/// Lowercase letters, digits and dashes, as Obsidian's ids: a folder name
/// on every platform.
pub fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && !id.starts_with('-')
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn check_id(id: &str) -> Result<(), String> {
    if valid_id(id) {
        Ok(())
    } else {
        Err(format!("{id} is not a plugin id"))
    }
}

fn meta_dir(root: &Path) -> PathBuf {
    root.join(".scratchnote")
}

pub fn plugins_dir(root: &Path) -> PathBuf {
    meta_dir(root).join(PLUGINS_DIR)
}

pub fn data_path(root: &Path, id: &str, core: bool) -> PathBuf {
    if core {
        meta_dir(root)
            .join(CORE_DATA_DIR)
            .join(format!("{id}.json"))
    } else {
        plugins_dir(root).join(id).join(DATA_FILE)
    }
}

/// `plugins.json`, or the defaults when it is missing or unreadable.
pub fn load_state(root: &Path) -> PluginState {
    std::fs::read_to_string(meta_dir(root).join(STATE_FILE))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Every installed community plugin: a folder named after a valid id, with
/// a manifest of that id and a `main.js`. Folders starting with a dot are
/// installs under way.
pub fn installed(root: &Path) -> Vec<Manifest> {
    let Ok(entries) = std::fs::read_dir(plugins_dir(root)) else {
        return Vec::new();
    };
    let mut found: Vec<Manifest> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            if !valid_id(&name) || !entry.path().join("main.js").is_file() {
                return None;
            }
            let raw = std::fs::read(entry.path().join("manifest.json")).ok()?;
            let manifest: Manifest = serde_json::from_slice(&raw).ok()?;
            (manifest.id == name).then_some(manifest)
        })
        .collect();
    found.sort_by_key(|manifest| manifest.name.to_lowercase());
    found
}

pub fn view(root: &Path) -> PluginsView {
    PluginsView {
        state: load_state(root),
        installed: installed(root),
    }
}

/// Ids made valid and unique, in the order given.
pub fn clean_ids(ids: Vec<String>) -> Result<Vec<String>, String> {
    let mut seen = Vec::new();
    for id in ids {
        check_id(&id)?;
        if !seen.contains(&id) {
            seen.push(id);
        }
    }
    Ok(seen)
}

pub async fn save_state(
    app: &AppHandle,
    state: &AppState,
    plugins: PluginState,
) -> Result<PluginsView, String> {
    let json = serde_json::to_string_pretty(&plugins).map_err(|e| e.to_string())?;
    state
        .writer
        .write_index(meta_dir(&state.root).join(STATE_FILE), json)
        .await?;
    Ok(changed(app, &state.root))
}

/// Tells every window what is installed and on now, so each loads and
/// unloads plugins to match.
pub fn changed(app: &AppHandle, root: &Path) -> PluginsView {
    let view = view(root);
    let _ = app.emit("plugins-changed", &view);
    view
}

/// Whether version `a` is older than `b`, compared number by number. A
/// pre-release suffix is ignored.
pub fn older(a: &str, b: &str) -> bool {
    let numbers = |v: &str| -> Vec<u64> {
        v.split(['-', '+'])
            .next()
            .unwrap_or("")
            .split('.')
            .map(|n| n.parse().unwrap_or(0))
            .collect()
    };
    let (a, b) = (numbers(a), numbers(b));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x < y;
        }
    }
    false
}

/// Writes the release into a folder of its own, then swaps it in for the
/// installed one, so a plugin is never half written. The old one's
/// `data.json` comes along.
pub fn put_in_place(dir: &Path, id: &str, release: &registry::Release) -> std::io::Result<()> {
    let target = dir.join(id);
    let staging = dir.join(format!(".{id}.new"));
    let old = dir.join(format!(".{id}.old"));
    for leftover in [&staging, &old] {
        if leftover.exists() {
            std::fs::remove_dir_all(leftover)?;
        }
    }
    std::fs::create_dir_all(&staging)?;
    std::fs::write(staging.join("manifest.json"), &release.manifest)?;
    std::fs::write(staging.join("main.js"), &release.main)?;
    if let Some(styles) = &release.styles {
        std::fs::write(staging.join("styles.css"), styles)?;
    }
    if target.exists() {
        let data = target.join(DATA_FILE);
        if data.is_file() {
            std::fs::copy(&data, staging.join(DATA_FILE))?;
        }
        std::fs::rename(&target, &old)?;
    }
    std::fs::rename(&staging, &target)?;
    if old.exists() {
        std::fs::remove_dir_all(&old)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("scratchnote-plugins-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn manifest(id: &str, version: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "id": id, "name": id, "version": version, "description": "", "author": ""
        }))
        .unwrap()
    }

    fn release(id: &str, version: &str, main: &str) -> registry::Release {
        registry::Release {
            manifest: manifest(id, version),
            main: main.as_bytes().to_vec(),
            styles: None,
        }
    }

    #[test]
    fn ids_are_folder_names_everywhere() {
        assert!(valid_id("highlights"));
        assert!(valid_id("word-count-2"));
        for bad in [
            "",
            "-x",
            "Tasks",
            "a b",
            "a/b",
            "..",
            "a.b",
            &"x".repeat(65),
        ] {
            assert!(!valid_id(bad), "{bad} should be refused");
        }
    }

    #[test]
    fn versions_compare_number_by_number() {
        assert!(older("0.2.0", "0.10.0"));
        assert!(older("1.0", "1.0.1"));
        assert!(!older("1.0.0", "1.0"));
        assert!(!older("1.2.0", "1.1.9"));
        assert!(!older("1.0.0-beta", "1.0.0"));
    }

    #[test]
    fn community_plugins_start_off() {
        let root = temp("state");
        assert_eq!(load_state(&root), PluginState::default());
        assert!(!load_state(&root).community);
    }

    #[test]
    fn a_state_file_without_core_enabled_still_reads() {
        let root = temp("older-state");
        std::fs::create_dir_all(meta_dir(&root)).unwrap();
        std::fs::write(
            meta_dir(&root).join(STATE_FILE),
            r#"{"community":true,"enabled":["x"],"coreDisabled":["basics"]}"#,
        )
        .unwrap();
        let state = load_state(&root);
        assert_eq!(state.core_disabled, vec!["basics"]);
        assert!(state.core_enabled.is_empty());
    }

    #[test]
    fn an_update_keeps_the_data_and_replaces_the_code() {
        let root = temp("update");
        let dir = plugins_dir(&root);
        put_in_place(&dir, "highlights", &release("highlights", "1.0.0", "one")).unwrap();
        std::fs::write(
            dir.join("highlights").join(DATA_FILE),
            "{\"color\":\"pink\"}",
        )
        .unwrap();
        std::fs::write(dir.join("highlights").join("styles.css"), "x").unwrap();

        put_in_place(&dir, "highlights", &release("highlights", "1.1.0", "two")).unwrap();

        let installed = installed(&root);
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].version, "1.1.0");
        let folder = dir.join("highlights");
        assert_eq!(
            std::fs::read_to_string(folder.join("main.js")).unwrap(),
            "two"
        );
        assert_eq!(
            std::fs::read_to_string(folder.join(DATA_FILE)).unwrap(),
            "{\"color\":\"pink\"}"
        );
        // The new release has no styles, so the old ones went with the old code.
        assert!(!folder.join("styles.css").exists());
        assert!(!dir.join(".highlights.old").exists());
        assert!(!dir.join(".highlights.new").exists());
    }

    #[test]
    fn only_well_formed_folders_count_as_installed() {
        let root = temp("installed");
        let dir = plugins_dir(&root);
        put_in_place(&dir, "good", &release("good", "1.0.0", "")).unwrap();
        // A manifest naming another id, a folder without code, an install
        // under way, and a name that is no id.
        put_in_place(&dir, "liar", &release("other", "1.0.0", "")).unwrap();
        std::fs::create_dir_all(dir.join("empty")).unwrap();
        std::fs::write(
            dir.join("empty").join("manifest.json"),
            manifest("empty", "1"),
        )
        .unwrap();
        put_in_place(&dir, ".hidden", &release(".hidden", "1.0.0", "")).unwrap();
        put_in_place(&dir, "Bad", &release("Bad", "1.0.0", "")).unwrap();

        let ids: Vec<_> = installed(&root).into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["good"]);
    }

    #[test]
    fn core_and_community_data_live_apart() {
        let root = Path::new("root");
        assert_eq!(
            data_path(root, "tasks", true),
            root.join(".scratchnote")
                .join("core-plugins")
                .join("tasks.json")
        );
        assert_eq!(
            data_path(root, "tasks", false),
            root.join(".scratchnote")
                .join("plugins")
                .join("tasks")
                .join("data.json")
        );
    }

    #[test]
    fn cleaned_ids_are_unique_and_valid() {
        assert_eq!(
            clean_ids(vec!["a".into(), "b".into(), "a".into()]).unwrap(),
            vec!["a", "b"]
        );
        assert!(clean_ids(vec!["../x".into()]).is_err());
    }
}
