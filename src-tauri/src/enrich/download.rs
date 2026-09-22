//! Fetching a GGUF model from Hugging Face, SPEC 5.2.
//!
//! Resumable, verified against the hash the registry reports, and recording
//! the exact repo revision so "which build do I have" has an answer later.
//! This and the user-initiated update check are the only network calls the
//! app ever makes.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use super::model::{model_file, models_dir, Variant};

/// What the registry says about the file we are about to fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteModel {
    /// Repo commit the file was resolved at. A quant repo is re-uploaded in
    /// place, so the filename alone is not an identity.
    pub revision: String,
    pub sha256: String,
    pub size: u64,
    pub url: String,
}

/// Written next to the .gguf so a later update check has something to compare.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstalledModel {
    pub repo: String,
    pub file: String,
    pub revision: String,
    pub sha256: String,
}

#[derive(Deserialize)]
struct RepoInfo {
    sha: String,
    siblings: Vec<Sibling>,
}

#[derive(Deserialize)]
struct Sibling {
    rfilename: String,
    lfs: Option<Lfs>,
}

#[derive(Deserialize)]
struct Lfs {
    /// The content hash. Not `oid`, and not the sibling's `blobId`, which is
    /// a git object id and says nothing about the bytes we download.
    sha256: String,
    size: u64,
}

pub fn sidecar_path(root: &Path, variant: Variant) -> PathBuf {
    models_dir(root).join(format!("{}.json", variant.file()))
}

pub fn installed(root: &Path, variant: Variant) -> Option<InstalledModel> {
    let raw = std::fs::read_to_string(sidecar_path(root, variant)).ok()?;
    serde_json::from_str(&raw).ok()
}

/// A model counts as present only when both the weights and the record of
/// what they are exist.
pub fn is_installed(root: &Path, variant: Variant) -> bool {
    model_file(root, variant).exists() && installed(root, variant).is_some()
}

/// The first variant with both weights and a record on disk.
pub fn installed_variant(root: &Path) -> Option<Variant> {
    [Variant::Default, Variant::Light]
        .into_iter()
        .find(|variant| is_installed(root, *variant))
}

/// Ask the registry which revision is current and what the file should hash to.
pub async fn lookup(variant: Variant) -> Result<RemoteModel, String> {
    let repo = variant.repo();
    let info: RepoInfo = reqwest::Client::new()
        .get(format!(
            "https://huggingface.co/api/models/{repo}?blobs=true"
        ))
        .send()
        .await
        .map_err(|e| format!("could not reach Hugging Face: {e}"))?
        .error_for_status()
        .map_err(|e| describe_lookup_failure(repo, &e))?
        .json()
        .await
        .map_err(|e| format!("unexpected answer from Hugging Face: {e}"))?;

    let wanted = variant.file();
    let sibling = info
        .siblings
        .iter()
        .find(|s| s.rfilename == wanted)
        .ok_or_else(|| format!("{repo} no longer publishes {wanted}"))?;
    let lfs = sibling
        .lfs
        .as_ref()
        .ok_or_else(|| format!("{wanted} has no hash to verify against"))?;

    Ok(RemoteModel {
        url: format!(
            "https://huggingface.co/{repo}/resolve/{}/{wanted}",
            info.sha
        ),
        revision: info.sha,
        sha256: lfs.sha256.clone(),
        size: lfs.size,
    })
}

/// Hugging Face answers 401 both for a private repo and for one that does not
/// exist, so the raw status is close to useless on its own.
fn describe_lookup_failure(repo: &str, e: &reqwest::Error) -> String {
    if e.status() == Some(reqwest::StatusCode::UNAUTHORIZED) {
        format!(
            "Hugging Face will not serve {repo}. It answers 401 both for repos              that need an account and for ones that do not exist, and a wrong              repo name is much the likelier of the two."
        )
    } else {
        format!("Hugging Face refused the request: {e}")
    }
}

/// Download to `<file>.part`, resuming whatever is already there, verify the
/// hash, then move it into place. The old file stays usable until the new one
/// has passed, so a failed update never leaves the app without a model.
pub async fn fetch<F>(
    root: &Path,
    variant: Variant,
    remote: &RemoteModel,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u8),
{
    let target = model_file(root, variant);
    let part = target.with_extension("gguf.part");
    tokio::fs::create_dir_all(models_dir(root))
        .await
        .map_err(|e| format!("could not create the models directory: {e}"))?;

    let mut done = tokio::fs::metadata(&part)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    if done > remote.size {
        // A leftover from a different revision; start again.
        let _ = tokio::fs::remove_file(&part).await;
        done = 0;
    }

    if done < remote.size {
        let mut request = reqwest::Client::new().get(&remote.url);
        if done > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={done}-"));
        }
        let response = request
            .send()
            .await
            .map_err(|e| format!("download failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("download refused: {e}"))?;

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&part)
            .await
            .map_err(|e| format!("could not open {}: {e}", part.display()))?;

        let mut stream = response.bytes_stream();
        let mut last_percent = u8::MAX;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("download interrupted: {e}"))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("could not write the model: {e}"))?;
            done += chunk.len() as u64;

            let percent = percent_of(done, remote.size);
            if percent != last_percent {
                last_percent = percent;
                on_progress(percent);
            }
        }
        file.sync_all()
            .await
            .map_err(|e| format!("could not flush the model: {e}"))?;
    }

    let actual = hash_file(&part).await?;
    if actual != remote.sha256 {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(format!(
            "the download did not match its hash (expected {}, got {actual})",
            remote.sha256
        ));
    }

    tokio::fs::rename(&part, &target)
        .await
        .map_err(|e| format!("could not move the model into place: {e}"))?;

    let record = InstalledModel {
        repo: variant.repo().to_string(),
        file: variant.file().to_string(),
        revision: remote.revision.clone(),
        sha256: remote.sha256.clone(),
    };
    let sidecar = serde_json::to_string_pretty(&record).map_err(|e| e.to_string())?;
    tokio::fs::write(sidecar_path(root, variant), sidecar)
        .await
        .map_err(|e| format!("could not record which model this is: {e}"))?;

    on_progress(100);
    Ok(())
}

pub fn percent_of(done: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    ((done.min(total) as f64 / total as f64) * 100.0).round() as u8
}

async fn hash_file(path: &Path) -> Result<String, String> {
    use tokio::io::AsyncReadExt;

    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| format!("could not reopen the download: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|e| format!("could not read the download: {e}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_a_percentage_that_cannot_overshoot() {
        assert_eq!(percent_of(0, 100), 0);
        assert_eq!(percent_of(50, 100), 50);
        assert_eq!(percent_of(100, 100), 100);
        assert_eq!(percent_of(150, 100), 100);
        assert_eq!(
            percent_of(1, 0),
            0,
            "an unknown size must not divide by zero"
        );
    }

    #[test]
    fn the_sidecar_sits_next_to_the_weights() {
        let root = Path::new("/root");
        let sidecar = sidecar_path(root, Variant::Light);
        assert_eq!(sidecar.parent(), model_file(root, Variant::Light).parent());
        assert!(sidecar.to_string_lossy().ends_with(".gguf.json"));
    }

    #[test]
    fn a_model_without_its_record_does_not_count_as_installed() {
        let root = std::env::temp_dir().join("scratchnote-model-record");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(models_dir(&root)).unwrap();
        std::fs::write(model_file(&root, Variant::Light), b"weights").unwrap();

        assert!(
            !is_installed(&root, Variant::Light),
            "weights alone say nothing about which revision they are"
        );

        std::fs::write(
            sidecar_path(&root, Variant::Light),
            serde_json::to_string(&InstalledModel {
                repo: Variant::Light.repo().to_string(),
                file: Variant::Light.file().to_string(),
                revision: "abc123".to_string(),
                sha256: "deadbeef".to_string(),
            })
            .unwrap(),
        )
        .unwrap();
        assert!(is_installed(&root, Variant::Light));
        assert_eq!(installed(&root, Variant::Light).unwrap().revision, "abc123");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Hits the network, so it is opt-in: `cargo test -- --ignored`.
    /// It is the only thing that can catch a repo being renamed or a quant
    /// being dropped, which otherwise shows up as a 401 in front of the user.
    #[tokio::test]
    #[ignore = "reaches Hugging Face"]
    async fn every_variant_resolves_on_hugging_face() {
        for variant in [Variant::Default, Variant::Light] {
            let remote = lookup(variant)
                .await
                .unwrap_or_else(|e| panic!("{} did not resolve: {e}", variant.repo()));

            assert_eq!(remote.sha256.len(), 64, "{:?} hash looks wrong", variant);
            assert!(remote.size > 0, "{:?} has no size", variant);
            assert!(
                remote.url.ends_with(variant.file()),
                "{:?} resolved to {}",
                variant,
                remote.url
            );
            assert!(!remote.revision.is_empty(), "{:?} has no revision", variant);
        }
    }

    #[tokio::test]
    async fn hashes_a_file_the_way_the_registry_reports_it() {
        let path = std::env::temp_dir().join("scratchnote-hash-test.bin");
        tokio::fs::write(&path, b"hello").await.unwrap();
        // Known SHA-256 of "hello".
        assert_eq!(
            hash_file(&path).await.unwrap(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        let _ = tokio::fs::remove_file(&path).await;
    }
}
