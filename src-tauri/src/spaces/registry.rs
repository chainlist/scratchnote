//! `.scratchnote/spaces.json` at the notes root: which space is open, and
//! how many notes each held when last left.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::FIRST_NAME;
use crate::storage::paths::meta_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Registry {
    /// Name of the open space.
    pub active: String,
    /// How many notes each space held when it was last left, for the
    /// switcher to show while the space is not open, and so not counted.
    pub notes: BTreeMap<String, usize>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            active: FIRST_NAME.to_string(),
            notes: BTreeMap::new(),
        }
    }
}

impl Registry {
    /// A missing or broken file opens the first space.
    pub fn load(root: &Path) -> Self {
        match std::fs::read_to_string(registry_path(root)) {
            Ok(raw) => serde_json::from_str::<Registry>(&raw).unwrap_or_else(|e| {
                log::warn!("spaces.json is not readable ({e}), opening the first space");
                Registry::default()
            }),
            Err(_) => Registry::default(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

pub fn registry_path(root: &Path) -> PathBuf {
    meta_dir(root).join("spaces.json")
}
