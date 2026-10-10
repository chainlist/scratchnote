//! Settings live at `~/Scratchnote/.scratchnote/settings.json` (SPEC 7),
//! wherever the notes root itself points.

use std::path::{Path, PathBuf};

use crate::Result;
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
    /// Name of the accent preset the windows paint buttons and focus rings in.
    pub accent_color: String,
    /// Name of the font preset the interface and notes are set in.
    pub font_family: String,
    /// Base text size in pixels; everything sized in rem scales with it.
    pub font_size: u32,
    /// Corner radius in rem.
    pub radius: f32,
    /// "dark", "light" or "system" to follow the OS.
    pub theme: String,
    /// A locale such as "fr", or "system" to follow the OS language. The
    /// frontend owns the list of locales; an unknown one follows the OS.
    pub language: String,
    /// The first-run walkthrough has been finished or skipped.
    pub onboarded: bool,
    /// The app version whose release notes were last shown. None from 0.1.0,
    /// which did not record it.
    pub last_seen_version: Option<String>,
    /// "oldest" or "newest": which of a thread's notes it lists first.
    pub thread_order: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            capture_hotkey: DEFAULT_HOTKEY.to_string(),
            hide_immediately: true,
            accent_color: "neutral".to_string(),
            font_family: "inter".to_string(),
            font_size: 16,
            radius: 0.625,
            theme: "dark".to_string(),
            language: "system".to_string(),
            onboarded: false,
            last_seen_version: None,
            thread_order: "oldest".to_string(),
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
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|e| {
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

    /// Check what the settings screen sent before anything is saved.
    pub fn validate(&self) -> Result<()> {
        if !self.root.is_absolute() {
            return Err(format!(
                "the notes folder must be a full path, not {}",
                self.root.display()
            )
            .into());
        }
        if self.capture_hotkey.trim().is_empty() {
            return Err("the capture hotkey cannot be empty".into());
        }
        Ok(())
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// The file `Settings::load` reads.
pub fn file(app: &AppHandle) -> PathBuf {
    settings_path(&default_root(app))
}

/// Android has no home folder anyone browses, so the notes start in the
/// app's own storage there; Settings can move them to a shared folder.
pub fn default_root(app: &AppHandle) -> PathBuf {
    #[cfg(mobile)]
    let home = app.path().app_data_dir();
    #[cfg(desktop)]
    let home = app.path().home_dir();
    match home {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> Settings {
        Settings {
            root: std::env::temp_dir().join("Scratchnote"),
            ..Settings::default()
        }
    }

    #[test]
    fn round_trips_through_json_in_camel_case() {
        let settings = Settings {
            hide_immediately: false,
            ..valid()
        };
        let json = settings.to_json();
        assert!(json.contains("\"captureHotkey\""));
        assert!(json.contains("\"hideImmediately\": false"));
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.root, settings.root);
        assert!(!back.hide_immediately);
    }

    #[test]
    fn a_file_missing_fields_falls_back_to_defaults() {
        let back: Settings = serde_json::from_str(r#"{"hideImmediately":false}"#).unwrap();
        assert_eq!(back.capture_hotkey, DEFAULT_HOTKEY);
        assert!(!back.hide_immediately);
        assert_eq!(back.accent_color, "neutral");
        assert_eq!(back.font_family, "inter");
        assert_eq!(back.font_size, 16);
        assert_eq!(back.theme, "dark");
        assert_eq!(back.language, "system");
        assert!(!back.onboarded);
        assert_eq!(back.last_seen_version, None);
        assert_eq!(back.thread_order, "oldest");
    }

    #[test]
    fn rejects_a_relative_root_and_an_empty_hotkey() {
        assert!(valid().validate().is_ok());
        let relative = Settings {
            root: PathBuf::from("notes"),
            ..valid()
        };
        assert!(relative.validate().is_err());
        let no_hotkey = Settings {
            capture_hotkey: "  ".to_string(),
            ..valid()
        };
        assert!(no_hotkey.validate().is_err());
    }
}
