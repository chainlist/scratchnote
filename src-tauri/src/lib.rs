mod chat;
mod commands;
mod enrich;
mod search;
mod settings;
mod spaces;
mod state;
mod storage;
mod watcher;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_global_shortcut::ShortcutState;

use enrich::download;
use enrich::{idle, worker};
use settings::Settings;
use state::{resting_status, AppState};
use storage::writer::Writer;

const CAPTURE: &str = "capture";
const MAIN: &str = "main";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_capture(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::save_note,
            commands::get_day,
            commands::list_days,
            commands::list_tags,
            commands::list_categories,
            commands::category_names,
            commands::search,
            commands::chat,
            commands::stop_chat,
            commands::warm_chat,
            commands::delete_note,
            commands::update_note,
            commands::update_note_meta,
            commands::rebuild_index,
            commands::regenerate_all,
            commands::retry_enrichment,
            commands::model_status,
            commands::enrich_busy,
            commands::enrich_progress,
            commands::model_info,
            commands::gpu_devices,
            commands::download_model,
            commands::check_model_update,
            commands::update_model,
            commands::today,
            commands::get_settings,
            commands::set_settings,
            commands::get_aliases,
            commands::set_aliases,
            commands::list_spaces,
            commands::create_space,
            commands::rename_space,
            commands::delete_space,
            commands::open_space_folder,
            commands::set_active_space,
            hide_capture,
            set_tray_labels,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let settings = Settings::load(app.handle());
            let hotkey = settings.capture_hotkey.clone();
            let root = settings.root.clone();

            // A model on disk is not loaded until the first job needs it
            // (SPEC 5.1), so "idle" rather than "loaded" at startup.
            let active = download::active_model(
                &root,
                settings.model_variant,
                settings.model_path.as_deref(),
            );
            let status = resting_status(settings.model_enabled, active.is_some());

            // Before spaces.json is read, since it may point it at a new folder.
            spaces::migrate_root(&root);

            let wake: worker::Wake = std::sync::Arc::new(tokio::sync::Notify::new());
            app.manage(AppState {
                root: root.clone(),
                settings: std::sync::RwLock::new(settings),
                writer: Writer::spawn(),
                // Filled in below: opening a space starts its watcher, which
                // needs the state to be managed already.
                spaces: std::sync::RwLock::new(Vec::new()),
                registry: std::sync::RwLock::new(spaces::Registry::load(&root)),
                backend: std::sync::RwLock::new(None),
                model_status: std::sync::RwLock::new(status),
                swapping: std::sync::atomic::AtomicBool::new(false),
                last_used: std::sync::Mutex::new(std::time::Instant::now()),
                busy: std::sync::atomic::AtomicBool::new(false),
                batch_done: std::sync::atomic::AtomicUsize::new(0),
                wake: wake.clone(),
            });

            // Every space is loaded, not just the open one, so notes captured
            // in any of them are enriched and their external edits noticed.
            let state = app.state::<AppState>();
            let mut found = spaces::discover(&root);
            if found.is_empty() {
                let path = spaces::spaces_dir(&root).join(spaces::FIRST_NAME);
                if let Err(e) = std::fs::create_dir_all(path.join("notes")) {
                    log::error!("could not create {}: {e}", path.display());
                }
                found.push((spaces::FIRST_NAME.to_string(), path));
            }
            let mut loaded: Vec<_> = found
                .into_iter()
                .map(|(name, path)| commands::open_space(app.handle(), &name, path))
                .collect();
            commands::sort_spaces(&mut loaded);
            if let Ok(mut spaces) = state.spaces.write() {
                *spaces = loaded;
            }

            worker::spawn(app.handle().clone(), wake.clone());
            idle::spawn(app.handle().clone());
            // Kick the worker in case a queue came back non-empty.
            wake.notify_one();

            build_capture_window(app.handle())?;
            build_tray(app.handle())?;
            keep_main_window_alive(app.handle());

            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                if let Err(e) = app.global_shortcut().register(hotkey.as_str()) {
                    log::error!("could not register the capture hotkey {hotkey}: {e}");
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Hiding from Rust keeps the capture webview free of window permissions.
#[tauri::command]
fn hide_capture(app: AppHandle) {
    if let Some(window) = app.get_webview_window(CAPTURE) {
        let _ = window.hide();
    }
}

/// The capture window is built once at startup and only ever shown and
/// hidden, so the hotkey never pays for window creation.
fn build_capture_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, CAPTURE, WebviewUrl::App("capture/".into()))
        .title("Scratchnote capture")
        .inner_size(620.0, 200.0)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .center()
        .build()
}

fn toggle_capture(app: &AppHandle) {
    let Some(window) = app.get_webview_window(CAPTURE) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        let _ = window.center();
        let _ = window.show();
        let _ = window.set_focus();
        // Tells the webview to focus the input; it is already loaded.
        let _ = app.emit_to(CAPTURE, "capture-shown", ());
    }
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Closing the main window hides it; the app stays in the tray.
fn keep_main_window_alive(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN) else {
        return;
    };
    let handle = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = handle.hide();
        }
    });
}

/// The tray menu's wording. The frontend holds the translations, so the
/// main window sends them once its language is known, and again on a change.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrayLabels {
    new_note: String,
    open: String,
    settings: String,
    quit: String,
}

impl Default for TrayLabels {
    fn default() -> Self {
        Self {
            new_note: "New note".into(),
            open: "Open Scratchnote".into(),
            settings: "Settings".into(),
            quit: "Quit".into(),
        }
    }
}

fn tray_menu(app: &AppHandle, labels: &TrayLabels) -> tauri::Result<Menu<tauri::Wry>> {
    let new_note = MenuItem::with_id(app, "new_note", &labels.new_note, true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", &labels.open, true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", &labels.settings, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", &labels.quit, true, None::<&str>)?;
    Menu::with_items(app, &[&new_note, &open, &settings, &quit])
}

#[tauri::command]
fn set_tray_labels(app: AppHandle, labels: TrayLabels) -> Result<(), String> {
    let tray = app.tray_by_id("tray").ok_or("no tray icon")?;
    let menu = tray_menu(&app, &labels).map_err(|e| e.to_string())?;
    tray.set_menu(Some(menu)).map_err(|e| e.to_string())
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = tray_menu(app, &TrayLabels::default())?;

    TrayIconBuilder::with_id("tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Scratchnote")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "new_note" => toggle_capture(app),
            "open" => show_main(app),
            "settings" => {
                show_main(app);
                let _ = app.emit_to(MAIN, "open-settings", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}
