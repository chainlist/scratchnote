//! The one error type of the backend.
//!
//! A command fails with the message alone: the webview shows `String(e)`
//! and never looks inside, so the error goes out as a plain string.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
    /// A thread panicked while it held this lock.
    #[error("{0} lock poisoned")]
    Poisoned(&'static str),
    /// The space was closed, or renamed or deleted, meanwhile.
    #[error("the space is closed")]
    SpaceClosed,
    /// Anything else, said for the user.
    #[error("{0}")]
    Msg(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::Msg(message)
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self::Msg(message.to_string())
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goes_out_as_its_message() {
        let json = serde_json::to_string(&Error::Poisoned("index")).unwrap();
        assert_eq!(json, r#""index lock poisoned""#);
        let json = serde_json::to_string(&Error::from("no note 01A")).unwrap();
        assert_eq!(json, r#""no note 01A""#);
    }
}
