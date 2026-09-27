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
    pub notes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpacesView {
    pub active: String,
    pub spaces: Vec<SpaceSummary>,
}

fn spaces_view(state: &AppState) -> Result<SpacesView, String> {
    let active = state.space()?.name.clone();
    let spaces = state
        .spaces
        .read()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .iter()
        .map(|s| SpaceSummary {
            name: s.name.clone(),
            notes: s.note_count(),
        })
        .collect();
    Ok(SpacesView { active, spaces })
}

/// Load a space and start watching it. Used at startup and whenever a space
/// is made or renamed.
pub fn open_space(app: &AppHandle, name: &str, root: PathBuf) -> Arc<Space> {
    let embed_wake = app.state::<AppState>().embed_wake.clone();
    let (space, refresh) = Space::open(name, root, embed_wake);
    let space = Arc::new(space);
    if let Err(e) = crate::watcher::start(app.clone(), &space) {
        log::error!("could not watch the notes of {name}: {e}");
    }
    let app = app.clone();
    let opened = space.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if refresh {
            if let Err(e) = opened.persist_index(&state.writer).await {
                log::warn!("could not write the index of {} back: {e}", opened.name);
            }
        }
        crate::pages::repair_stubs(&state.writer, &opened).await;
    });
    space
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

    let space = open_space(&app, &name, root);
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
    spaces_changed(&app, &state).await
}

/// Open another space. Every note command acts on it from then on, and the
/// capture window saves into it.
#[tauri::command]
pub async fn set_active_space(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<SpacesView, String> {
    if state.find_space(&name).is_none() {
        return Err(format!("no space {name}"));
    }
    state
        .registry
        .write()
        .map_err(|_| "spaces lock poisoned".to_string())?
        .active = name;
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
    old.retire();
    let to = spaces::spaces_dir(&state.root).join(&new_name);
    if let Err(e) = tokio::fs::rename(&old.root, &to).await {
        let back = open_space(&app, &name, old.root.clone());
        replace_space(&state, &old, back)?;
        return Err(format!("could not rename {}: {e}", old.root.display()));
    }
    let renamed = open_space(&app, &new_name, to);
    replace_space(&state, &old, renamed)?;

    {
        let mut registry = state
            .registry
            .write()
            .map_err(|_| "spaces lock poisoned".to_string())?;
        if registry.active == name {
            registry.active = new_name;
        }
    }
    // A job the old space had in hand was dropped; its note is still pending
    // and was queued again by the reopened space.
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

    space.retire();
    let trash = state.root.join(".scratchnote").join("trash");
    let to = trash.join(format!("{name} {}", Local::now().format("%Y-%m-%d %H%M%S")));
    let moved = match tokio::fs::create_dir_all(&trash).await {
        Ok(()) => tokio::fs::rename(&space.root, &to).await,
        Err(e) => Err(e),
    };
    if let Err(e) = moved {
        let back = open_space(&app, &name, space.root.clone());
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
        .map(|s| s.name.clone());
    {
        let mut registry = state
            .registry
            .write()
            .map_err(|_| "spaces lock poisoned".to_string())?;
        if let (true, Some(first)) = (registry.active == name, first) {
            registry.active = first;
        }
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
