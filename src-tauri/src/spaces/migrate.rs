//! Moving the notes kept at the root, from before every space had a
//! folder, into a space of their own.

use std::path::Path;

use serde::Deserialize;

use super::{check_name, discover, registry_path, spaces_dir, Registry, FIRST_NAME};
use crate::storage::index;
use crate::storage::paths::{self, first_free, meta_dir};

/// Before every space had a folder, the first one lived at the notes root,
/// named in `spaces.json`. Move its notes and its own files into
/// `spaces/<its name>/`, once, and point `spaces.json` at the new folder.
/// Settings, models and the trash stay at the root, shared by every space.
pub fn migrate_root(root: &Path) {
    let notes = root.join("notes");
    if !notes.is_dir() {
        return;
    }
    #[derive(Default, Deserialize)]
    #[serde(default, rename_all = "camelCase")]
    struct Old {
        active: String,
        default_name: String,
    }
    let old: Old = std::fs::read_to_string(registry_path(root))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    let wanted = check_name(&old.default_name).unwrap_or_else(|_| FIRST_NAME.to_string());
    // Folders compare without case on Windows, so "personal" would clash too.
    let taken: Vec<String> = discover(root)
        .into_iter()
        .map(|(name, _)| name.to_lowercase())
        .collect();
    let name = first_free(
        |n| paths::numbered(&wanted, None, n),
        |name| taken.contains(&name.to_lowercase()),
    );
    let to = spaces_dir(root).join(&name);

    let moved = std::fs::create_dir_all(meta_dir(&to))
        .and_then(|_| std::fs::rename(&notes, to.join("notes")));
    if let Err(e) = moved {
        log::error!(
            "could not move {} into {}: {e}",
            notes.display(),
            to.display()
        );
        return;
    }
    let from = index::index_path(root);
    if from.exists() {
        if let Err(e) = std::fs::rename(&from, index::index_path(&to)) {
            log::warn!(
                "could not move {} into {}: {e}",
                from.display(),
                to.display()
            );
        }
    }

    let active = if old.active.is_empty() || old.active == old.default_name {
        name.clone()
    } else {
        old.active
    };
    let registry = Registry {
        active,
        ..Registry::default()
    };
    if let Err(e) = std::fs::write(registry_path(root), registry.to_json()) {
        log::warn!("could not update spaces.json ({e})");
    }
    log::info!("moved the notes at the root into spaces/{name}");
}
