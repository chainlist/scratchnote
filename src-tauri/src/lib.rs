mod attachments;
mod chat;
mod commands;
mod embed;
mod enrich;
mod pages;
mod plugins;
mod search;
mod settings;
mod spaces;
mod state;
mod storage;
mod watcher;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{
    AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};
use tauri_plugin_global_shortcut::ShortcutState;

use enrich::download;
use enrich::{idle, worker};
use settings::Settings;
use state::{resting_status, AppState};
use storage::writer::Writer;

const CAPTURE: &str = "capture";
const MAIN: &str = "main";
/// Toggles the capture window, for desktops where apps cannot register a
/// global shortcut (Wayland): the user binds this command there instead.
const CAPTURE_FLAG: &str = "--capture";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Launching the app again reaches the running one instead, which also
        // brings the main window back on desktops that show no tray icon.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if args.iter().any(|a| a == CAPTURE_FLAG) {
                toggle_capture(app);
            } else {
                show_main(app);
            }
        }))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_capture(app);
                    }
                })
                .build(),
        )
        // Attached images, read from the open space's `attachments/` folder
        // (SPEC 3.7). The folder is chosen at runtime, which a Tauri asset
        // scope in the config could not follow.
        .register_asynchronous_uri_scheme_protocol(
            attachments::SCHEME,
            |ctx, request, responder| {
                let app = ctx.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    responder.respond(attachments::serve(app, request).await);
                });
            },
        )
        .invoke_handler(tauri::generate_handler![
            commands::notes::save_note,
            commands::notes::get_day,
            commands::notes::list_days,
            commands::search::list_categories,
            commands::search::category_names,
            commands::search::search,
            commands::search::search_meaning,
            commands::search::similar_notes,
            commands::search::recall,
            commands::threads::list_threads,
            commands::threads::get_thread,
            commands::threads::rename_thread,
            commands::threads::keep_out_of_threads,
            commands::search::notes_containing,
            commands::chat::chat,
            commands::chat::stop_chat,
            commands::chat::warm_chat,
            commands::notes::delete_note,
            commands::notes::update_note,
            commands::notes::update_note_meta,
            commands::notes::rebuild_index,
            commands::notes::regenerate_all,
            commands::notes::retry_enrichment,
            commands::notes::move_note,
            commands::models::model_status,
            commands::models::enrich_busy,
            commands::models::enrich_progress,
            commands::models::model_info,
            commands::models::benchmark_model,
            commands::models::gpu_devices,
            commands::models::system_profile,
            commands::models::download_model,
            commands::models::embedding_model_info,
            commands::models::download_embedding_model,
            commands::models::check_model_update,
            commands::models::update_model,
            commands::notes::today,
            commands::settings::get_settings,
            commands::settings::set_settings,
            commands::settings::restart_app,
            commands::spaces::list_spaces,
            commands::spaces::create_space,
            commands::spaces::rename_space,
            commands::spaces::delete_space,
            commands::spaces::open_space_folder,
            commands::notes::open_link,
            commands::spaces::set_active_space,
            commands::pages::create_page,
            commands::pages::list_pages,
            commands::pages::get_page,
            commands::pages::update_page,
            commands::pages::finish_page,
            commands::pages::rename_page,
            commands::pages::delete_page,
            commands::pages::note_to_page,
            commands::pages::move_page,
            commands::attachments::add_attachments,
            commands::attachments::save_attachment,
            commands::attachments::move_attachments,
            commands::attachments::open_attachment,
            commands::plugins::plugins_view,
            commands::plugins::set_plugins,
            commands::plugins::plugin_code,
            commands::plugins::plugin_data,
            commands::plugins::save_plugin_data,
            commands::plugins::browse_plugins,
            commands::plugins::plugin_details,
            commands::plugins::install_plugin,
            commands::plugins::uninstall_plugin,
            hide_capture,
            capture_to_page,
            reveal_note,
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
            let embed_wake: worker::Wake = std::sync::Arc::new(tokio::sync::Notify::new());
            app.manage(AppState {
                root: root.clone(),
                settings: std::sync::RwLock::new(settings),
                writer: Writer::spawn(),
                // Filled in below: opening a space starts its watcher, which
                // needs the state to be managed already.
                spaces: std::sync::RwLock::new(Vec::new()),
                registry: std::sync::RwLock::new(spaces::Registry::load(&root)),
                backend: std::sync::RwLock::new(None),
                backend_loading: std::sync::Mutex::new(()),
                model_status: std::sync::RwLock::new(status),
                swapping: std::sync::atomic::AtomicBool::new(false),
                last_used: std::sync::Mutex::new(std::time::Instant::now()),
                busy: std::sync::atomic::AtomicBool::new(false),
                batch_done: std::sync::atomic::AtomicUsize::new(0),
                wake: wake.clone(),
                embedder: std::sync::RwLock::new(None),
                embedder_loading: std::sync::Mutex::new(()),
                embed_wake: embed_wake.clone(),
                embedding_download: std::sync::Mutex::new(None),
            });

            // Every space is listed, so the notes left in any space's queue
            // are still enriched, but only the open one is read and watched
            // (SPEC 4.6). The others are read when they are opened.
            let state = app.state::<AppState>();
            let mut found = spaces::discover(&root);
            if found.is_empty() {
                let path = spaces::spaces_dir(&root).join(spaces::FIRST_NAME);
                if let Err(e) = std::fs::create_dir_all(path.join("notes")) {
                    log::error!("could not create {}: {e}", path.display());
                }
                found.push((spaces::FIRST_NAME.to_string(), path));
            }
            let mut listed: Vec<_> = found
                .into_iter()
                .map(|(name, path)| commands::spaces::add_space(app.handle(), &name, path))
                .collect();
            commands::spaces::sort_spaces(&mut listed);
            if let Ok(mut spaces) = state.spaces.write() {
                *spaces = listed;
            }
            if let Ok(open) = state.space() {
                commands::spaces::load_space(app.handle(), &open);
            }

            worker::spawn(app.handle().clone(), wake.clone());
            // Opening the space above already woke it, so its notes are
            // backfilled.
            embed::sync::spawn(app.handle().clone(), embed_wake);
            idle::spawn(app.handle().clone());
            commands::models::replace_legacy_embedding_model(app.handle());
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

            if std::env::args().any(|a| a == CAPTURE_FLAG) {
                toggle_capture(app.handle());
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(on_run_event);
}

/// Clicking the Dock icon brings back a main window that closing hid.
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
fn on_run_event(app: &AppHandle, event: tauri::RunEvent) {
    #[cfg(target_os = "macos")]
    if let tauri::RunEvent::Reopen { .. } = event {
        show_main(app);
    }
}

/// Hiding from Rust keeps the capture webview free of window permissions
/// beyond dragging itself.
#[tauri::command]
fn hide_capture(app: AppHandle) {
    if let Some(window) = app.get_webview_window(CAPTURE) {
        let _ = window.hide();
    }
}

/// Hand the capture window's draft to a new page in the main window (SPEC
/// 3.5): the capture window goes, the main window comes up and opens a page
/// with the draft as its text. A draft meant for a space not open opens
/// that space first, since the main window writes pages into the open one;
/// its `spaces-changed` reaches the main window ahead of `new-page`.
#[tauri::command]
async fn capture_to_page(
    app: AppHandle,
    state: State<'_, AppState>,
    body: String,
    space: Option<String>,
) -> Result<(), String> {
    if let Some(space) = space {
        commands::spaces::set_active_space(app.clone(), state, space).await?;
    }
    hide_capture(app.clone());
    show_main(&app);
    let _ = app.emit_to(MAIN, "new-page", serde_json::json!({ "body": body }));
    Ok(())
}

/// Show a note of the open space in the main window, for the capture
/// window's recall (SPEC 6.3): the capture window goes, keeping its draft as
/// Esc does, and the main window comes up on the note.
#[tauri::command]
fn reveal_note(app: AppHandle, id: String, date: String, kind: Option<String>) {
    hide_capture(app.clone());
    show_main(&app);
    let _ = app.emit_to(
        MAIN,
        "reveal-note",
        serde_json::json!({ "id": id, "date": date, "kind": kind }),
    );
}

/// The capture window is built once at startup and only ever shown and
/// hidden, so the hotkey never pays for window creation. It opens centred;
/// where the user moves and sizes it then holds until the app quits.
fn build_capture_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, CAPTURE, WebviewUrl::App("capture/".into()))
        .title("Scratchnote capture")
        .inner_size(620.0, 200.0)
        .min_inner_size(420.0, 150.0)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .maximizable(false)
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
