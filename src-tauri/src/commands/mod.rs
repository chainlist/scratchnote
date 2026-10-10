//! Tauri commands, SPEC 8.
//!
//! A command that reads files, takes a space's locks or asks SQLite is
//! async: Tauri runs a sync command on the main thread, where waiting on a
//! rebuild or a map pass would freeze the windows.

pub mod attachments;
pub mod labels;
pub mod map;
pub mod mentions;
pub mod models;
pub mod notes;
pub mod pages;
pub mod pins;
pub mod plugins;
pub mod search;
pub mod settings;
pub mod spaces;
pub mod threads;
