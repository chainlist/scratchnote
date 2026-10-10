//! Plugins, SPEC 3.9 and 4.9: what the settings and the webview ask of
//! them. How they are kept is `crate::plugins`.

use tauri::{AppHandle, Emitter, State, Window};

use super::blocking;
use crate::events;
use crate::plugins::{
    check_id, clean_ids, data_path, load_state, older, plugins_dir, put_in_place, registry,
    save_state, view, Manifest, PluginCode, PluginState, PluginsView,
};
use crate::state::AppState;
use crate::Result;

fn check_community(state: &AppState) -> Result<()> {
    if load_state(&state.root).community {
        Ok(())
    } else {
        Err("community plugins are turned off".into())
    }
}

/// What is installed and on now, told to every window, so each loads and
/// unloads plugins to match.
fn changed(app: &AppHandle, state: &AppState) -> PluginsView {
    let view = view(&state.root);
    events::plugins_changed(app, &view);
    view
}

/// Save `plugins.json`, and tell every window.
async fn save(app: &AppHandle, state: &AppState, plugins: PluginState) -> Result<PluginsView> {
    save_state(&state.writer, &state.root, &plugins).await?;
    Ok(changed(app, state))
}

#[tauri::command]
pub async fn plugins_view(state: State<'_, AppState>) -> Result<PluginsView> {
    Ok(view(&state.root))
}

/// Save which plugins are on. Every window hears of it and follows.
#[tauri::command]
pub async fn set_plugins(
    app: AppHandle,
    state: State<'_, AppState>,
    plugins: PluginState,
) -> Result<PluginsView> {
    let plugins = PluginState {
        community: plugins.community,
        enabled: clean_ids(plugins.enabled)?,
        core_disabled: clean_ids(plugins.core_disabled)?,
        core_enabled: clean_ids(plugins.core_enabled)?,
    };
    save(&app, &state, plugins).await
}

/// An enabled community plugin's code. Nothing is handed out while
/// community plugins are off, whatever the webview asks.
#[tauri::command]
pub async fn plugin_code(state: State<'_, AppState>, id: String) -> Result<PluginCode> {
    check_id(&id)?;
    let plugins = load_state(&state.root);
    if !plugins.community || !plugins.enabled.contains(&id) {
        return Err(format!("{id} is not switched on").into());
    }
    let dir = plugins_dir(&state.root).join(&id);
    let main = std::fs::read_to_string(dir.join("main.js"))
        .map_err(|e| format!("could not read the code of {id}: {e}"))?;
    let styles = std::fs::read_to_string(dir.join("styles.css")).ok();
    Ok(PluginCode { main, styles })
}

/// What a plugin saved, or null before it ever did.
#[tauri::command]
pub async fn plugin_data(
    state: State<'_, AppState>,
    id: String,
    core: bool,
) -> Result<Option<serde_json::Value>> {
    check_id(&id)?;
    match std::fs::read_to_string(data_path(&state.root, &id, core)) {
        Ok(raw) => serde_json::from_str(&raw)
            .map(Some)
            .map_err(|e| format!("the data of {id} is not readable: {e}").into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Save a plugin's data. The other windows hear of it, so their copy of the
/// plugin can read it again.
#[tauri::command]
pub async fn save_plugin_data(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    id: String,
    core: bool,
    data: serde_json::Value,
) -> Result<()> {
    check_id(&id)?;
    if !core && !plugins_dir(&state.root).join(&id).is_dir() {
        return Err(format!("{id} is not installed").into());
    }
    let json = serde_json::to_string_pretty(&data)?;
    state
        .writer
        .write(data_path(&state.root, &id, core), json)
        .await?;
    let _ = app.emit(
        "plugin-data-changed",
        serde_json::json!({ "id": id, "window": window.label() }),
    );
    Ok(())
}

/// Every plugin the registry lists.
#[tauri::command]
pub async fn browse_plugins(state: State<'_, AppState>) -> Result<Vec<registry::Entry>> {
    check_community(&state)?;
    registry::Source::current()?.list().await
}

/// A plugin's latest manifest and its README.
#[tauri::command]
pub async fn plugin_details(state: State<'_, AppState>, repo: String) -> Result<registry::Details> {
    check_community(&state)?;
    registry::Source::current()?.details(&repo).await
}

/// Download the latest release of the plugin `id` from `repo` and install
/// it, over an older one if there is one. Its data is kept. Enabling it is
/// left to the user.
#[tauri::command]
pub async fn install_plugin(
    app: AppHandle,
    state: State<'_, AppState>,
    repo: String,
    id: String,
) -> Result<PluginsView> {
    check_id(&id)?;
    check_community(&state)?;
    let source = registry::Source::current()?;
    let latest = source.latest(&repo).await?;
    if latest.id != id {
        return Err(format!("{repo} holds the plugin {}, not {id}", latest.id).into());
    }
    let app_version = app.package_info().version.to_string();
    if let Some(needed) = &latest.min_app_version {
        if older(&app_version, needed) {
            return Err(format!("{} needs Scratchnote {needed} or newer", latest.name).into());
        }
    }
    let release = source.release(&repo, &latest.version).await?;
    let manifest: Manifest = serde_json::from_slice(&release.manifest)
        .map_err(|e| format!("the released manifest is not readable: {e}"))?;
    if manifest.id != id || manifest.version != latest.version {
        return Err(format!(
            "release {} of {repo} does not match its manifest",
            latest.version
        )
        .into());
    }
    let root = state.root.clone();
    blocking(move || put_in_place(&plugins_dir(&root), &id, &release))
        .await?
        .map_err(|e| format!("could not install the plugin: {e}"))?;
    Ok(changed(&app, &state))
}

/// Remove a community plugin, its data with it, and switch it off.
#[tauri::command]
pub async fn uninstall_plugin(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<PluginsView> {
    check_id(&id)?;
    let dir = plugins_dir(&state.root).join(&id);
    if dir.exists() {
        blocking(move || std::fs::remove_dir_all(dir))
            .await?
            .map_err(|e| format!("could not remove {id}: {e}"))?;
    }
    let mut plugins = load_state(&state.root);
    plugins.enabled.retain(|enabled| *enabled != id);
    save(&app, &state, plugins).await
}
