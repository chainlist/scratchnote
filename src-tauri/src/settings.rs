//! Settings live at `~/Scratchnote/.scratchnote/settings.json` (SPEC 7),
//! wherever the notes root itself points.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::enrich::model::Variant;

pub const DEFAULT_HOTKEY: &str = "CommandOrControl+Shift+Space";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub root: PathBuf,
    pub capture_hotkey: String,
    /// Hide the capture window the moment a note is saved, rather than
    /// showing a "Saved" toast first.
    pub hide_immediately: bool,
    /// Which of the app's own models to use.
    pub model_variant: Variant,
    /// A GGUF file of the user's own, used instead of `model_variant`.
    pub model_path: Option<PathBuf>,
    /// Unload the model after this many minutes without a job (SPEC 5.1).
    /// Zero keeps it loaded.
    pub idle_unload_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            capture_hotkey: DEFAULT_HOTKEY.to_string(),
            hide_immediately: true,
            model_variant: Variant::Default,
            model_path: None,
            idle_unload_minutes: 10,
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

    /// Check what the settings screen sent before anything is saved.
    pub fn validate(&self) -> Result<(), String> {
        if !self.root.is_absolute() {
            return Err(format!(
                "the notes folder must be a full path, not {}",
                self.root.display()
            ));
        }
        if self.capture_hotkey.trim().is_empty() {
            return Err("the capture hotkey cannot be empty".to_string());
        }
        if let Some(path) = &self.model_path {
            let is_gguf = path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"));
            if !path.is_file() || !is_gguf {
                return Err(format!("{} is not a .gguf file", path.display()));
            }
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

    #[test]
    fn a_custom_model_must_be_an_existing_gguf_file() {
        let dir = std::env::temp_dir().join("scratchnote-settings-gguf");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let with = |path: PathBuf| Settings {
            model_path: Some(path),
            ..valid()
        };

        assert!(with(dir.join("missing.gguf")).validate().is_err());
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        assert!(with(dir.join("notes.txt")).validate().is_err());
        std::fs::write(dir.join("mine.GGUF"), b"x").unwrap();
        assert!(with(dir.join("mine.GGUF")).validate().is_ok());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
