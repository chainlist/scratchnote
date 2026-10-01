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
    /// Off keeps the model out of memory entirely, for machines too small
    /// for it: notes are queued but not enriched, and chat is unavailable.
    pub model_enabled: bool,
    /// Which of the app's own models to use.
    pub model_variant: Variant,
    /// A GGUF file of the user's own, used instead of `model_variant`.
    pub model_path: Option<PathBuf>,
    /// Unload the chat model after this many seconds without a job or a
    /// chat (SPEC 5.1). Zero keeps it loaded. The embedding model is never
    /// unloaded for being idle.
    pub idle_unload_seconds: u32,
    /// Run the model on the GPU when one is available.
    pub use_gpu: bool,
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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            capture_hotkey: DEFAULT_HOTKEY.to_string(),
            hide_immediately: true,
            // Phones are the low end the switch exists for.
            model_enabled: !cfg!(mobile),
            model_variant: Variant::Default,
            model_path: None,
            // The default model takes a few seconds to load, too long to pay
            // for every short pause between notes.
            idle_unload_seconds: 600,
            use_gpu: true,
            accent_color: "neutral".to_string(),
            font_family: "inter".to_string(),
            font_size: 16,
            radius: 0.625,
            theme: "dark".to_string(),
            language: "system".to_string(),
            onboarded: false,
            last_seen_version: None,
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
            Ok(raw) => Self::parse(&raw).unwrap_or_else(|e| {
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

    /// Settings from the text of settings.json, with what an older build
    /// wrote under another name carried over.
    fn parse(raw: &str) -> serde_json::Result<Self> {
        let value: serde_json::Value = serde_json::from_str(raw)?;
        let mut settings = Self::deserialize(&value)?;
        // The idle unload was counted in minutes until it took seconds.
        if value.get("idleUnloadSeconds").is_none() {
            if let Some(minutes) = value.get("idleUnloadMinutes").and_then(|m| m.as_u64()) {
                settings.idle_unload_seconds =
                    u32::try_from(minutes.saturating_mul(60)).unwrap_or(u32::MAX);
            }
        }
        Ok(settings)
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
        assert_eq!(back.model_enabled, !cfg!(mobile));
        assert_eq!(back.accent_color, "neutral");
        assert_eq!(back.font_family, "inter");
        assert_eq!(back.font_size, 16);
        assert_eq!(back.theme, "dark");
        assert_eq!(back.language, "system");
        assert!(!back.onboarded);
        assert_eq!(back.last_seen_version, None);
    }

    #[test]
    fn an_idle_unload_in_minutes_is_read_as_seconds() {
        let old = Settings::parse(r#"{"idleUnloadMinutes":5}"#).unwrap();
        assert_eq!(old.idle_unload_seconds, 300);
        let kept = Settings::parse(r#"{"idleUnloadMinutes":0}"#).unwrap();
        assert_eq!(kept.idle_unload_seconds, 0);
        // Seconds, once written, win over a leftover minutes.
        let new = Settings::parse(r#"{"idleUnloadMinutes":5,"idleUnloadSeconds":45}"#).unwrap();
        assert_eq!(new.idle_unload_seconds, 45);
        assert_eq!(Settings::parse("{}").unwrap().idle_unload_seconds, 600);
        assert!(!Settings::default().to_json().contains("idleUnloadMinutes"));
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
