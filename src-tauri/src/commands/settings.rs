//! Settings, SPEC 7.

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::settings::Settings;
use crate::state::AppState;

/// Settings as saved, plus the root this run is actually using, so the screen
/// can say when a new root is waiting on a restart.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    #[serde(flatten)]
    pub settings: Settings,
    pub active_root: std::path::PathBuf,
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<SettingsView, String> {
    Ok(SettingsView {
        settings: current_settings(&state)?,
        active_root: state.root.clone(),
    })
}

/// Save settings (SPEC 7). The hotkey switches straight away; a new notes
/// root is saved but only used from the next launch.
#[tauri::command]
pub async fn set_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<SettingsView, String> {
    settings.validate()?;
    let old = current_settings(&state)?;

    if settings.capture_hotkey != old.capture_hotkey {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        let shortcuts = app.global_shortcut();
        // Bind the new one first, so a key the OS refuses leaves the old one
        // working instead of leaving no hotkey at all.
        shortcuts
            .register(settings.capture_hotkey.as_str())
            .map_err(|e| format!("cannot use {} as the hotkey: {e}", settings.capture_hotkey))?;
        let _ = shortcuts.unregister(old.capture_hotkey.as_str());
    }

    save_settings(&app, &state, settings.clone()).await?;
    Ok(SettingsView {
        settings,
        active_root: state.root.clone(),
    })
}

/// Relaunch the app, which is how a new notes root takes effect. The
/// onboarding uses it so the model downloads into the chosen folder.
#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.restart();
}

pub(super) fn current_settings(state: &State<'_, AppState>) -> Result<Settings, String> {
    Ok(state
        .settings
        .read()
        .map_err(|_| "settings lock poisoned".to_string())?
        .clone())
}

pub(super) async fn save_settings(
    app: &AppHandle,
    state: &State<'_, AppState>,
    settings: Settings,
) -> Result<(), String> {
    state
        .writer
        .write_index(crate::settings::file(app), settings.to_json())
        .await?;
    *state
        .settings
        .write()
        .map_err(|_| "settings lock poisoned".to_string())? = settings.clone();
    // The capture window reads hideImmediately from this.
    let _ = app.emit("settings-changed", &settings);
    Ok(())
}
