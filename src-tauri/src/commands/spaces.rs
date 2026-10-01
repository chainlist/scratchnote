//! Spaces, SPEC 4.6.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::spaces::{self, Space};
use crate::state::AppState;

/// One row of the space switcher.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceSummary {
    pub name: String,
    /// `None` for a space not open since before its count was recorded,
    /// such as a folder made by hand.
    pub notes: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpacesView {
    pub active: String,
    pub spaces: Vec<SpaceSummary>,
}

fn spaces_view(state: &AppState) -> Result<SpacesView, String> {
    let active = state.space()?.name.clone();
    let registry = state
        .registry
        .read()
        .map_err(|_| "spaces lock poisoned".to_string())?;
    let spaces = state
        .spaces
        .read()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .iter()
        .map(|s| SpaceSummary {
            name: s.name.clone(),
            notes: s
                .note_count()
                .or_else(|| registry.notes.get(&s.name).copied()),
        })
        .collect();
    Ok(SpacesView { active, spaces })
}

/// A space as listed, not open yet: only its queue is read (SPEC 4.6).
pub fn add_space(app: &AppHandle, name: &str, root: PathBuf) -> Arc<Space> {
    let embed_wake = app.state::<AppState>().embed_wake.clone();
    Arc::new(Space::new(name, root, embed_wake))
}

/// Open a space: read its notes and start watching it. At startup for the
/// open space, and whenever another one is opened.
pub fn load_space(app: &AppHandle, space: &Arc<Space>) {
    let refresh = space.load();
    if let Err(e) = crate::watcher::start(app.clone(), space) {
        log::error!("could not watch the notes of {}: {e}", space.name);
    }
    if !refresh {
        return;
    }
    let app = app.clone();
    let opened = space.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(e) = opened.persist_index(&state.writer).await {
            log::warn!("could not write the index of {} back: {e}", opened.name);
        }
    });
}

/// `load_space` off the async runtime, since it reads every file the cache
/// does not cover.
async fn load_space_off_thread(app: &AppHandle, space: &Arc<Space>) -> Result<(), String> {
    let (app, space) = (app.clone(), space.clone());
    tauri::async_runtime::spawn_blocking(move || load_space(&app, &space))
        .await
        .map_err(|e| e.to_string())
}

/// Close the space being left, and note how many notes it held for the
/// switcher to show meanwhile. A page left open there is let go of with it.
async fn close_space(state: &AppState, space: &Space) -> Result<(), String> {
    crate::pages::log_open_edits(&state.writer, space).await;
    if let Some(count) = space.unload() {
        state
            .registry
            .write()
            .map_err(|_| "spaces lock poisoned".to_string())?
            .notes
            .insert(space.name.clone(), count);
    }
    Ok(())
}

/// Record the registry and tell both windows the spaces changed.
async fn spaces_changed(app: &AppHandle, state: &AppState) -> Result<SpacesView, String> {
    let json = state
        .registry
        .read()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .to_json();
    state
        .writer
        .write_index(spaces::registry_path(&state.root), json)
        .await?;
    let view = spaces_view(state)?;
    let _ = app.emit("spaces-changed", &view);
    Ok(view)
}

/// A name no other space has, compared the way Windows compares folder names.
fn unused_name(state: &AppState, raw: &str, renaming: Option<&str>) -> Result<String, String> {
    let name = spaces::check_name(raw)?;
    let taken = state
        .spaces
        .read()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .iter()
        .any(|s| Some(s.name.as_str()) != renaming && s.name.to_lowercase() == name.to_lowercase());
    if taken {
        return Err(format!("there is already a space called {name}"));
    }
    Ok(name)
}

/// Swap a space for its replacement, keeping them in name order.
fn replace_space(state: &AppState, old: &Arc<Space>, new: Arc<Space>) -> Result<(), String> {
    let mut spaces = state
        .spaces
        .write()
        .map_err(|_| "spaces lock poisoned".to_string())?;
    if let Some(slot) = spaces.iter_mut().find(|s| Arc::ptr_eq(s, old)) {
        *slot = new;
    }
    sort_spaces(&mut spaces);
    Ok(())
}

pub fn sort_spaces(spaces: &mut [Arc<Space>]) {
    spaces.sort_by_key(|s| s.name.to_lowercase());
}

#[tauri::command]
pub fn list_spaces(state: State<'_, AppState>) -> Result<SpacesView, String> {
    spaces_view(&state)
}

/// Make a space and open it. Its folder is `spaces/<name>/` under the root.
#[tauri::command]
pub async fn create_space(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<SpacesView, String> {
    let name = unused_name(&state, &name, None)?;
    let root = spaces::spaces_dir(&state.root).join(&name);
    if root.exists() {
        return Err(format!("{} is already there", root.display()));
    }
    tokio::fs::create_dir_all(root.join("notes"))
        .await
        .map_err(|e| format!("could not create {}: {e}", root.display()))?;

    let space = add_space(&app, &name, root);
    load_space_off_thread(&app, &space).await?;
    let left = state.space()?;
    {
        let mut spaces = state
            .spaces
            .write()
            .map_err(|_| "spaces lock poisoned".to_string())?;
        spaces.push(space);
        sort_spaces(&mut spaces);
    }
    state
        .registry
        .write()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .active = name;
    close_space(&state, &left).await?;
    spaces_changed(&app, &state).await
}

/// Open another space. Every note command acts on it from then on, and the
/// capture window saves into it. The space left is closed: its notes leave
/// memory, and its queue stays with the worker.
#[tauri::command]
pub async fn set_active_space(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<SpacesView, String> {
    let space = state
        .find_space(&name)
        .ok_or_else(|| format!("no space {name}"))?;
    let left = state.space()?;
    if Arc::ptr_eq(&space, &left) {
        return spaces_view(&state);
    }
    if !space.is_open() {
        load_space_off_thread(&app, &space).await?;
    }
    state
        .registry
        .write()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .active = name;
    close_space(&state, &left).await?;
    // Its notes are now first in line for the model.
    state.wake.notify_one();
    spaces_changed(&app, &state).await
}

/// Rename a space, and with it its folder.
#[tauri::command]
pub async fn rename_space(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    new_name: String,
) -> Result<SpacesView, String> {
    let old = state
        .find_space(&name)
        .ok_or_else(|| format!("no space {name}"))?;
    let new_name = unused_name(&state, &new_name, Some(&name))?;
    if new_name == name {
        return spaces_view(&state);
    }

    // The watcher holds the folder open, and Windows will not move an open
    // folder, so the old space lets go of it first.
    let was_open = old.is_open();
    old.retire();
    let to = spaces::spaces_dir(&state.root).join(&new_name);
    if let Err(e) = tokio::fs::rename(&old.root, &to).await {
        let back = add_space(&app, &name, old.root.clone());
        if was_open {
            load_space_off_thread(&app, &back).await?;
        }
        replace_space(&state, &old, back)?;
        return Err(format!("could not rename {}: {e}", old.root.display()));
    }
    let renamed = add_space(&app, &new_name, to);
    if was_open {
        load_space_off_thread(&app, &renamed).await?;
    }
    replace_space(&state, &old, renamed)?;

    {
        let mut registry = state
            .registry
            .write()
            .map_err(|_| "spaces lock poisoned".to_string())?;
        if registry.active == name {
            registry.active = new_name.clone();
        }
        if let Some(count) = registry.notes.remove(&name) {
            registry.notes.insert(new_name, count);
        }
    }
    // A job the old space had in hand was dropped; its note is still pending
    // and is queued again when the space is next opened.
    state.wake.notify_one();
    spaces_changed(&app, &state).await
}

/// Take a space out of the app. Its folder is moved to `.scratchnote/trash/`
/// rather than deleted, so its notes can be recovered by moving the folder
/// back under `spaces/`.
#[tauri::command]
pub async fn delete_space(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<SpacesView, String> {
    let space = state
        .find_space(&name)
        .ok_or_else(|| format!("no space {name}"))?;
    let last = state
        .spaces
        .read()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .len()
        <= 1;
    if last {
        return Err("the last space cannot be deleted".to_string());
    }

    let was_open = space.is_open();
    space.retire();
    let trash = state.root.join(".scratchnote").join("trash");
    let to = trash.join(format!("{name} {}", Local::now().format("%Y-%m-%d %H%M%S")));
    let moved = match tokio::fs::create_dir_all(&trash).await {
        Ok(()) => tokio::fs::rename(&space.root, &to).await,
        Err(e) => Err(e),
    };
    if let Err(e) = moved {
        let back = add_space(&app, &name, space.root.clone());
        if was_open {
            load_space_off_thread(&app, &back).await?;
        }
        replace_space(&state, &space, back)?;
        return Err(format!(
            "could not move {} to the trash: {e}",
            space.root.display()
        ));
    }

    state
        .spaces
        .write()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .retain(|s| !Arc::ptr_eq(s, &space));
    let first = state
        .spaces
        .read()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .first()
        .cloned();
    // The open space went: the first one opens in its place.
    let reopen = {
        let mut registry = state
            .registry
            .write()
            .map_err(|_| "spaces lock poisoned".to_string())?;
        registry.notes.remove(&name);
        match first {
            Some(first) if registry.active == name => {
                registry.active = first.name.clone();
                Some(first)
            }
            _ => None,
        }
    };
    if let Some(first) = reopen.filter(|first| !first.is_open()) {
        load_space_off_thread(&app, &first).await?;
    }
    spaces_changed(&app, &state).await
}

/// Show a space's folder in the system file manager. The path is looked up
/// here rather than passed in, so the page can only open folders that are
/// spaces.
#[tauri::command]
pub fn open_space_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let space = state
        .find_space(&name)
        .ok_or_else(|| format!("no space {name}"))?;
    app.opener()
        .open_path(space.root.to_string_lossy(), None::<&str>)
        .map_err(|e| format!("could not open {}: {e}", space.root.display()))
}
