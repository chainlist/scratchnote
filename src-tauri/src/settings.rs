//! Settings live at `<root>/.scratchnote/settings.json`. Milestone 1 only
//! reads them; the editor comes with the settings screen.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

pub const DEFAULT_HOTKEY: &str = "CommandOrControl+Shift+Space";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub root: PathBuf,
    pub capture_hotkey: String,
    /// Hide the capture window the moment a note is saved, rather than
    /// showing a "Saved" toast first.
    pub hide_immediately: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            capture_hotkey: DEFAULT_HOTKEY.to_string(),
            hide_immediately: true,
        }
    }
}

impl Settings {
    /// Read settings from the default root. A `root` recorded there wins, so
    /// the notes directory can point anywhere while the file that says so
    /// stays in one predictable place.
    pub fn load(app: &AppHandle) -> Self {
        let default_root = default_root(app);
        let mut settings = match std::fs::read_to_string(settings_path(&default_root)) {
            Ok(raw) => serde_json::from_str::<Settings>(&raw).unwrap_or_else(|e| {
                log::warn!("settings.json is not readable ({e}), falling back to defaults");
                Settings::default()
            }),
            Err(_) => Settings::default(),
        };
        if settings.root.as_os_str() == "." {
            settings.root = default_root;
        }
        settings
    }
}

fn default_root(app: &AppHandle) -> PathBuf {
    match app.path().home_dir() {
        Ok(home) => home.join("Scratchnote"),
        Err(e) => {
            log::warn!("no home directory ({e}), using the working directory");
            PathBuf::from("Scratchnote")
        }
    }
}

fn settings_path(root: &Path) -> PathBuf {
    root.join(".scratchnote").join("settings.json")
}
