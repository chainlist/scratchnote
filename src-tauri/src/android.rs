//! What Android needs from its own Java side: choosing the notes folder,
//! which the dialog plugin cannot do there, and relaunching the app, which
//! Tauri's restart cannot. `StoragePlugin.kt` in `gen/android` answers.

use crate::Result;
use serde::Deserialize;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{AppHandle, Manager, Wry};

struct Storage(PluginHandle<Wry>);

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("storage")
        .setup(|app, api| {
            let handle = api.register_android_plugin("com.scratchnote.app", "StoragePlugin")?;
            app.manage(Storage(handle));
            Ok(())
        })
        .build()
}

#[derive(Deserialize)]
struct Picked {
    path: Option<String>,
}

/// The folder the user chose, as a full path, once the app may write
/// there. None when they back out.
pub async fn pick_folder(app: &AppHandle) -> Result<Option<String>> {
    let picked: Picked = app
        .state::<Storage>()
        .0
        .run_mobile_plugin_async("pickFolder", ())
        .await?;
    Ok(picked.path)
}

/// Not waited on: the call runs on Android's main thread, which a command
/// may be holding, and the process ends with it anyway.
pub fn restart(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let storage = app.state::<Storage>();
        if let Err(e) = storage
            .0
            .run_mobile_plugin_async::<serde_json::Value>("restart", ())
            .await
        {
            log::error!("could not relaunch the app: {e}");
        }
    });
}
