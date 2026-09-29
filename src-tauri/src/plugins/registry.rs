//! Where community plugins come from (SPEC 4.10), as in Obsidian: one
//! GitHub repository lists them in `community-plugins.json`, each plugin's
//! own repository holds its `manifest.json` and `README.md`, and its GitHub
//! release for a version holds the files that are installed.
//!
//! A local folder can stand in for GitHub. It mirrors the URLs, host first
//! (`<folder>/raw.githubusercontent.com/<owner>/<repo>/HEAD/manifest.json`),
//! so the app asks a folder for exactly what it would ask GitHub for, and a
//! folder that works is a GitHub layout that works.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{valid_id, Manifest};

/// The repository that lists every community plugin.
pub const REPO: &str = "chainlist/scratchnote-plugins";
/// Points the app at a local folder instead of GitHub, in any build.
pub const FOLDER_VAR: &str = "SCRATCHNOTE_PLUGIN_REGISTRY";
/// No plugin file is anywhere near this; past it, something is wrong.
const MAX_FILE: usize = 10 * 1024 * 1024;

/// One line of `community-plugins.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    /// `owner/name` on GitHub.
    pub repo: String,
}

/// What the details of a plugin show before it is installed.
#[derive(Debug, Serialize)]
pub struct Details {
    /// The manifest at the repository's head: the latest version.
    pub manifest: Manifest,
    pub readme: Option<String>,
}

/// The files of one release, as they are installed.
pub struct Release {
    pub manifest: Vec<u8>,
    pub main: Vec<u8>,
    pub styles: Option<Vec<u8>>,
}

pub enum Source {
    GitHub(reqwest::Client),
    Folder(PathBuf),
}

impl Source {
    /// A folder named by `SCRATCHNOTE_PLUGIN_REGISTRY` if there is one.
    /// Otherwise a dev build reads the repository's own `plugin-registry/`
    /// and a release build reads GitHub.
    pub fn current() -> Result<Self, String> {
        if let Some(dir) = std::env::var_os(FOLDER_VAR) {
            return Ok(Source::Folder(PathBuf::from(dir)));
        }
        if cfg!(debug_assertions) {
            return Ok(Source::Folder(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join("plugin-registry"),
            ));
        }
        let client = reqwest::Client::builder()
            .user_agent(concat!("Scratchnote/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Source::GitHub(client))
    }

    /// Every plugin the registry lists. Lines that are not well formed are
    /// left out rather than failing the whole list.
    pub async fn list(&self) -> Result<Vec<Entry>, String> {
        let raw = self
            .get(&raw_url(REPO, "community-plugins.json"))
            .await?
            .ok_or("the plugin registry has no community-plugins.json")?;
        let entries: Vec<serde_json::Value> = serde_json::from_slice(&raw)
            .map_err(|e| format!("community-plugins.json is not readable: {e}"))?;
        Ok(entries
            .into_iter()
            .filter_map(|entry| serde_json::from_value::<Entry>(entry).ok())
            .filter(|entry| valid_id(&entry.id) && valid_repo(&entry.repo))
            .collect())
    }

    pub async fn details(&self, repo: &str) -> Result<Details, String> {
        check_repo(repo)?;
        let manifest = self.latest(repo).await?;
        let readme = self
            .get(&raw_url(repo, "README.md"))
            .await?
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
        Ok(Details { manifest, readme })
    }

    /// The manifest at the head of a plugin's repository, which names its
    /// latest version.
    pub async fn latest(&self, repo: &str) -> Result<Manifest, String> {
        check_repo(repo)?;
        let raw = self
            .get(&raw_url(repo, "manifest.json"))
            .await?
            .ok_or_else(|| format!("{repo} has no manifest.json"))?;
        serde_json::from_slice(&raw)
            .map_err(|e| format!("the manifest of {repo} is not readable: {e}"))
    }

    /// The files a release carries. `styles.css` is optional.
    pub async fn release(&self, repo: &str, version: &str) -> Result<Release, String> {
        check_repo(repo)?;
        if !valid_version(version) {
            return Err(format!("{version} is not a version"));
        }
        let file = |name: &str| release_url(repo, version, name);
        let missing = |name: &str| format!("release {version} of {repo} has no {name}");
        Ok(Release {
            manifest: self
                .get(&file("manifest.json"))
                .await?
                .ok_or_else(|| missing("manifest.json"))?,
            main: self
                .get(&file("main.js"))
                .await?
                .ok_or_else(|| missing("main.js"))?,
            styles: self.get(&file("styles.css")).await?,
        })
    }

    /// The file at `url`, or None when it is not there.
    async fn get(&self, url: &str) -> Result<Option<Vec<u8>>, String> {
        match self {
            Source::GitHub(client) => {
                let response = client
                    .get(url)
                    .send()
                    .await
                    .map_err(|e| format!("could not reach GitHub: {e}"))?;
                if response.status() == reqwest::StatusCode::NOT_FOUND {
                    return Ok(None);
                }
                let response = response
                    .error_for_status()
                    .map_err(|e| format!("GitHub refused {url}: {e}"))?;
                if response.content_length().unwrap_or(0) > MAX_FILE as u64 {
                    return Err(format!("{url} is too large"));
                }
                let bytes = response.bytes().await.map_err(|e| e.to_string())?;
                if bytes.len() > MAX_FILE {
                    return Err(format!("{url} is too large"));
                }
                Ok(Some(bytes.to_vec()))
            }
            Source::Folder(dir) => {
                let path = local_path(dir, url)?;
                match tokio::fs::metadata(&path).await {
                    Ok(meta) if meta.len() > MAX_FILE as u64 => {
                        return Err(format!("{} is too large", path.display()))
                    }
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(e) => return Err(format!("{}: {e}", path.display())),
                }
                tokio::fs::read(&path)
                    .await
                    .map(Some)
                    .map_err(|e| format!("{}: {e}", path.display()))
            }
        }
    }
}

/// A file at the head of a repository's default branch.
pub fn raw_url(repo: &str, file: &str) -> String {
    format!("https://raw.githubusercontent.com/{repo}/HEAD/{file}")
}

/// A file attached to a repository's release for `version`.
pub fn release_url(repo: &str, version: &str, file: &str) -> String {
    format!("https://github.com/{repo}/releases/download/{version}/{file}")
}

/// Where a local registry keeps the file GitHub serves at `url`: the host,
/// then the path, one folder per segment. Every segment was validated on
/// the way in; this checks again, so no URL reaches outside the folder.
fn local_path(dir: &Path, url: &str) -> Result<PathBuf, String> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| format!("not an https URL: {url}"))?;
    let mut path = dir.to_path_buf();
    for segment in rest.split('/') {
        if !safe_segment(segment) {
            return Err(format!("not a registry URL: {url}"));
        }
        path.push(segment);
    }
    Ok(path)
}

/// A folder or file name that stays where it is put.
fn safe_segment(segment: &str) -> bool {
    !segment.is_empty()
        && !segment.starts_with('.')
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-+".contains(c))
}

/// `owner/name`, as GitHub spells a repository.
pub fn valid_repo(repo: &str) -> bool {
    let mut parts = repo.split('/');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(owner), Some(name), None) if safe_segment(owner) && safe_segment(name)
    )
}

fn check_repo(repo: &str) -> Result<(), String> {
    if valid_repo(repo) {
        Ok(())
    } else {
        Err(format!("{repo} is not a GitHub repository"))
    }
}

/// A release tag: `1.0.0`, `1.2.0-beta.1`.
pub fn valid_version(version: &str) -> bool {
    safe_segment(version) && !version.contains("..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repositories_are_owner_and_name() {
        assert!(valid_repo("chainlist/scratchnote-highlights"));
        assert!(valid_repo("a.b/c_d"));
        for bad in [
            "",
            "chainlist",
            "a/b/c",
            "../b",
            "a/..",
            "a/.git",
            "a /b",
            "a\\b/c",
        ] {
            assert!(!valid_repo(bad), "{bad} should be refused");
        }
    }

    #[test]
    fn versions_cannot_climb_out_of_the_release() {
        assert!(valid_version("1.0.0"));
        assert!(valid_version("1.2.0-beta.1"));
        for bad in ["", "..", "1..2", ".1", "1/2", "1\\2"] {
            assert!(!valid_version(bad), "{bad} should be refused");
        }
    }

    #[test]
    fn a_folder_mirrors_the_github_urls_host_first() {
        let dir = Path::new("registry");
        assert_eq!(
            local_path(dir, &raw_url("a/b", "manifest.json")).unwrap(),
            dir.join("raw.githubusercontent.com")
                .join("a")
                .join("b")
                .join("HEAD")
                .join("manifest.json")
        );
        assert_eq!(
            local_path(dir, &release_url("a/b", "1.0.0", "main.js")).unwrap(),
            dir.join("github.com")
                .join("a")
                .join("b")
                .join("releases")
                .join("download")
                .join("1.0.0")
                .join("main.js")
        );
        assert!(local_path(dir, "https://github.com/a/../../x").is_err());
        assert!(local_path(dir, "http://github.com/a/b").is_err());
    }
}
