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
    /// Where the notes go when no folder is chosen, for Android's "app
    /// storage" choice.
    pub default_root: std::path::PathBuf,
}

#[tauri::command]
pub fn get_settings(app: AppHandle, state: State<'_, AppState>) -> Result<SettingsView, String> {
    Ok(SettingsView {
        settings: current_settings(&state)?,
        active_root: state.root.clone(),
        default_root: crate::settings::default_root(&app),
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

    #[cfg(desktop)]
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

    #[cfg(mobile)]
    let _ = old;

    save_settings(&app, &state, settings.clone()).await?;
    Ok(SettingsView {
        settings,
        active_root: state.root.clone(),
        default_root: crate::settings::default_root(&app),
    })
}

/// Relaunch the app, which is how a new notes root takes effect. The
/// onboarding uses it so the model downloads into the chosen folder.
#[tauri::command]
pub fn restart_app(app: AppHandle) {
    // Tauri restarts by running its own binary again, which Android does
    // not allow, so the activity is relaunched there instead.
    #[cfg(target_os = "android")]
    crate::android::restart(&app);
    #[cfg(not(target_os = "android"))]
    app.restart();
}

/// Android's folder chooser for the notes root, as a full path: the dialog
/// plugin cannot pick folders there. None when the user backs out.
#[tauri::command]
pub async fn pick_notes_folder(app: AppHandle) -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    return crate::android::pick_folder(&app).await;
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Err("the dialog plugin picks folders on this system".to_string())
    }
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
