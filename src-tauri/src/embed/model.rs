//! The embedding model the app fetches, and where it lives.

use crate::Result;
use std::path::PathBuf;

/// A model the app can fetch from Hugging Face: where it is published and
/// the file it is saved as.
pub trait Catalogued: Copy {
    fn repo(self) -> &'static str;
    fn file(self) -> &'static str;
}

/// EmbeddingGemma-300M, which turns notes into vectors for search by
/// meaning. One model, so nothing to choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddingModel;

impl Catalogued for EmbeddingModel {
    /// Google's own repo is gated behind a licence click, which a plain
    /// download cannot do; ggml-org publishes the same weights openly, about
    /// 330 MB at Q8_0.
    fn repo(self) -> &'static str {
        "ggml-org/embeddinggemma-300M-GGUF"
    }

    fn file(self) -> &'static str {
        "embeddinggemma-300M-Q8_0.gguf"
    }
}

/// The embedding model the app shipped before, removed once its successor is
/// in.
pub const LEGACY_EMBEDDING_FILE: &str = "Qwen3-Embedding-0.6B-Q8_0.gguf";

/// Where a downloaded model lives, under `local_data`, the app's own folder
/// on this computer, so a sync tool carrying the notes root never carries
/// the model (SPEC 4.1).
pub fn models_dir(local_data: &std::path::Path) -> PathBuf {
    local_data.join("models")
}

/// Move the models versions up to 0.7.0 kept in the notes root's
/// `models/` into `local_data`'s, once: each file is renamed, or copied and
/// then removed when the two are on different drives. A file already in the
/// new folder is left where it was. The old folder goes once empty. Returns
/// whether anything moved.
pub fn move_from_notes_root(root: &std::path::Path, local_data: &std::path::Path) -> bool {
    let (from, to) = (models_dir(root), models_dir(local_data));
    let Ok(files) = std::fs::read_dir(&from) else {
        return false;
    };
    if from == to {
        return false;
    }
    if let Err(e) = std::fs::create_dir_all(&to) {
        log::warn!("could not create {}: {e}", to.display());
        return false;
    }
    let mut moved = false;
    for file in files.flatten() {
        let (source, target) = (file.path(), to.join(file.file_name()));
        if !source.is_file() || target.exists() {
            continue;
        }
        let done = std::fs::rename(&source, &target).or_else(|_| {
            // Copied under another name first, so a copy cut short is never
            // taken for the model.
            let part = to.join(format!("{}.moving", file.file_name().to_string_lossy()));
            std::fs::copy(&source, &part)?;
            std::fs::rename(&part, &target)?;
            std::fs::remove_file(&source)
        });
        match done {
            Ok(()) => {
                log::info!("moved {} to {}", source.display(), target.display());
                moved = true;
            }
            Err(e) => log::warn!("could not move {}: {e}", source.display()),
        }
    }
    let _ = std::fs::remove_dir(&from);
    moved
}

/// The app's own folder on this computer, as Tauri's `app_local_data_dir`
/// names it, for the tests that use the downloaded model.
#[cfg(test)]
pub fn installed_local_data() -> Option<PathBuf> {
    let var = |name| std::env::var_os(name).map(PathBuf::from);
    let base = if cfg!(windows) {
        var("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|home| home.join("Library/Application Support"))
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|home| home.join(".local/share")))
    };
    Some(base?.join("com.scratchnote.app"))
}

pub fn model_file(local_data: &std::path::Path, model: impl Catalogued) -> PathBuf {
    models_dir(local_data).join(model.file())
}

/// The chat models versions before 0.5.0 downloaded, which nothing reads
/// any more (SPEC 4.5).
const OLD_CHAT_FILES: [&str; 2] = [
    "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
    "Qwen3-1.7B-Q4_K_M.gguf",
];

/// Each old chat model in `models/` and a download of it not finished.
fn old_chat_models(root: &std::path::Path) -> impl Iterator<Item = PathBuf> + '_ {
    OLD_CHAT_FILES.iter().flat_map(move |file| {
        let model = models_dir(root).join(file);
        [model.with_extension("gguf.part"), model]
    })
}

/// How many bytes the old chat models left in `models/` take, `None` when
/// none is there, whatever records of their downloads are left.
pub fn old_chat_bytes(root: &std::path::Path) -> Option<u64> {
    let sizes: Vec<u64> = old_chat_models(root)
        .filter_map(|path| std::fs::metadata(path).ok())
        .map(|meta| meta.len())
        .collect();
    (!sizes.is_empty()).then(|| sizes.iter().sum())
}

/// Remove the old chat models, the downloads of them not finished and the
/// records of their downloads. A file already gone is no matter; the first
/// that cannot be removed says why, once every other was tried.
pub fn remove_old_chat_models(root: &std::path::Path) -> Result<()> {
    let records = OLD_CHAT_FILES
        .iter()
        .map(|file| models_dir(root).join(format!("{file}.json")));
    let mut failed = None;
    for path in old_chat_models(root).chain(records) {
        match std::fs::remove_file(&path) {
            Ok(()) => log::info!("removed {}", path.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                failed.get_or_insert(format!("could not remove {}: {e}", path.display()));
            }
        }
    }
    failed.map_or(Ok(()), |e| Err(e.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These are the exact paths the download hits. A typo here is a 401 at
    /// runtime and nothing earlier, because Hugging Face answers 401 for a
    /// repo that does not exist just as it does for a private one.
    #[test]
    fn the_catalog_points_at_the_repo_that_actually_publishes_the_file() {
        assert_eq!(EmbeddingModel.repo(), "ggml-org/embeddinggemma-300M-GGUF");
        assert_eq!(EmbeddingModel.file(), "embeddinggemma-300M-Q8_0.gguf");
    }

    #[test]
    fn the_models_in_the_notes_root_move_out_of_it_once() {
        let root = scratch("move-models-root");
        let local = scratch("move-models-local");
        let old = models_dir(&root);
        std::fs::write(model_file(&root, EmbeddingModel), b"weights").unwrap();
        std::fs::write(old.join(format!("{}.json", EmbeddingModel.file())), b"{}").unwrap();
        // Already in the new folder, so it stays where it is.
        std::fs::write(old.join("kept.gguf"), b"old").unwrap();
        std::fs::write(models_dir(&local).join("kept.gguf"), b"new").unwrap();

        assert!(move_from_notes_root(&root, &local));
        assert_eq!(
            std::fs::read(model_file(&local, EmbeddingModel)).unwrap(),
            b"weights"
        );
        assert!(models_dir(&local)
            .join(format!("{}.json", EmbeddingModel.file()))
            .is_file());
        assert_eq!(
            std::fs::read(models_dir(&local).join("kept.gguf")).unwrap(),
            b"new"
        );
        assert!(!model_file(&root, EmbeddingModel).exists());
        assert!(
            old.join("kept.gguf").is_file(),
            "the folder is not empty, so it stays"
        );

        std::fs::remove_file(old.join("kept.gguf")).unwrap();
        assert!(!move_from_notes_root(&root, &local));
        assert!(!old.exists(), "an empty old folder goes");
        assert!(!move_from_notes_root(&root, &local), "nothing left to move");
    }

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("scratchnote-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(models_dir(&root)).unwrap();
        root
    }

    #[test]
    fn the_old_chat_models_are_found_and_removed_and_nothing_else() {
        let root = scratch("old-chat-models");
        let models = models_dir(&root);
        let chat = models.join("Qwen3-4B-Instruct-2507-Q4_K_M.gguf");
        std::fs::write(&chat, [0u8; 300]).unwrap();
        std::fs::write(chat.with_extension("gguf.json"), b"{}").unwrap();
        std::fs::write(models.join("Qwen3-1.7B-Q4_K_M.gguf.part"), [0u8; 20]).unwrap();
        std::fs::write(model_file(&root, EmbeddingModel), b"kept").unwrap();

        assert_eq!(old_chat_bytes(&root), Some(320));
        remove_old_chat_models(&root).unwrap();
        assert_eq!(old_chat_bytes(&root), None);
        let left: Vec<String> = std::fs::read_dir(&models)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, vec![EmbeddingModel.file()]);
    }

    #[test]
    fn only_the_records_of_old_downloads_count_as_no_chat_model() {
        let root = scratch("old-chat-records");
        let models = models_dir(&root);
        std::fs::write(models.join("Qwen3-1.7B-Q4_K_M.gguf.json"), b"{}").unwrap();
        assert_eq!(old_chat_bytes(&root), None);
    }
}
