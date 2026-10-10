mod ahead;
#[cfg(target_os = "android")]
mod android;
mod attachments;
mod commands;
#[cfg(desktop)]
mod desktop;
mod embed;
mod error;
mod events;
mod http;
mod mentions;
mod pages;
mod plugins;
mod search;
mod settings;
mod spaces;
mod startup;
mod state;
mod storage;
mod watcher;

pub(crate) use error::{Error, Result};

use std::sync::{Arc, Mutex, RwLock};

use tauri::{App, AppHandle, Emitter, Manager, State};

use embed::sync::Wake;
use settings::Settings;
use state::AppState;
use storage::writer::Writer;

const CAPTURE: &str = "capture";
const MAIN: &str = "main";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    startup::begin();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());
    #[cfg(desktop)]
    let builder = desktop::plugins(builder);
    #[cfg(target_os = "android")]
    let builder = builder.plugin(android::init());
    builder
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
            // Notes and the days they are on.
            commands::notes::save_note,
            commands::notes::get_day,
            commands::notes::list_days,
            commands::notes::get_note,
            commands::notes::get_notes,
            commands::notes::delete_note,
            commands::notes::update_note,
            commands::notes::move_note,
            commands::notes::notes_about,
            commands::notes::clear_day_ahead,
            commands::notes::rebuild_index,
            commands::notes::today,
            commands::notes::open_link,
            // Pages.
            commands::pages::create_page,
            commands::pages::list_pages,
            commands::pages::get_page,
            commands::pages::update_page,
            commands::pages::finish_page,
            commands::pages::rename_page,
            commands::pages::delete_page,
            commands::pages::note_to_page,
            commands::pages::move_page,
            // Search, recall and what notes are about.
            commands::search::search,
            commands::search::search_meaning,
            commands::search::similar_notes,
            commands::search::draft_hints,
            commands::search::notes_containing,
            commands::labels::note_labels,
            // Threads, mentions, pins and the map.
            commands::threads::list_threads,
            commands::threads::get_thread,
            commands::threads::rename_thread,
            commands::threads::keep_out_of_threads,
            commands::threads::keep_thread,
            commands::threads::dismiss_thread,
            commands::threads::put_in_thread,
            commands::threads::merge_threads,
            commands::threads::undo_thread_change,
            commands::threads::threads_for_note,
            commands::threads::thread_cards,
            commands::mentions::list_mentions,
            commands::mentions::notes_mentioning,
            commands::pins::list_pins,
            commands::pins::set_pins,
            commands::map::note_map,
            commands::map::map_categories,
            commands::map::map_links,
            commands::map::map_search,
            // Attachments.
            commands::attachments::add_attachments,
            commands::attachments::save_attachment,
            commands::attachments::move_attachments,
            commands::attachments::open_attachment,
            // Spaces.
            commands::spaces::list_spaces,
            commands::spaces::create_space,
            commands::spaces::rename_space,
            commands::spaces::delete_space,
            commands::spaces::open_space_folder,
            commands::spaces::set_active_space,
            // The embedding model.
            commands::models::embedding_model_info,
            commands::models::download_embedding_model,
            commands::models::old_chat_model,
            commands::models::remove_old_chat_model,
            commands::models::embedder_activity,
            // Settings and plugins.
            commands::settings::get_settings,
            commands::settings::set_settings,
            commands::settings::restart_app,
            commands::settings::pick_notes_folder,
            commands::plugins::plugins_view,
            commands::plugins::set_plugins,
            commands::plugins::plugin_code,
            commands::plugins::plugin_data,
            commands::plugins::save_plugin_data,
            commands::plugins::browse_plugins,
            commands::plugins::plugin_details,
            commands::plugins::install_plugin,
            commands::plugins::uninstall_plugin,
            // The windows.
            hide_capture,
            capture_to_page,
            reveal_note,
            #[cfg(desktop)]
            desktop::set_tray_labels,
            #[cfg(mobile)]
            set_tray_labels,
            startup::launch_steps,
        ])
        .setup(setup)
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(on_run_event);
}

/// Everything the app does at launch before its window loads, step by step
/// (`startup`).
fn setup(app: &mut App) -> std::result::Result<(), Box<dyn std::error::Error>> {
    startup::step("Starting Tauri and its plugins");
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

    // Before spaces.json is read, since it may point it at a new folder.
    spaces::migrate_root(&root);
    startup::step("Reading the settings");
    let local_data = app.path().app_local_data_dir().unwrap_or_else(|e| {
        log::warn!("no folder for the app's own data ({e}), keeping the models in the notes root");
        root.clone()
    });

    let embed_wake: Wake = Arc::new(tokio::sync::Notify::new());
    app.manage(AppState {
        root: root.clone(),
        local_data,
        settings: RwLock::new(settings),
        writer: Writer::spawn(),
        // Filled in below: opening a space starts its watcher, which needs
        // the state to be managed already.
        spaces: RwLock::new(Vec::new()),
        registry: RwLock::new(spaces::Registry::load(&root)),
        embedder: RwLock::new(None),
        embedder_loading: Mutex::new(()),
        embed_wake: embed_wake.clone(),
        embedding_download: Mutex::new(None),
        embedder_activity: Mutex::default(),
    });

    open_spaces(app.handle());
    start_embedding(app.handle(), embed_wake);

    #[cfg(desktop)]
    desktop::setup(app.handle(), &hotkey)?;
    #[cfg(mobile)]
    let _ = hotkey;
    startup::done();
    Ok(())
}

/// List every space, but read and watch only the open one (SPEC 4.6). The
/// others are read when they are opened.
fn open_spaces(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut listed: Vec<_> = spaces::discover_or_first(&state.root)
        .into_iter()
        .map(|(name, path)| commands::spaces::add_space(app, &name, path))
        .collect();
    commands::spaces::sort_spaces(&mut listed);
    if let Ok(mut spaces) = state.spaces.write() {
        *spaces = listed;
    }
    startup::step("Listing the spaces");
    if let Ok(open) = state.space() {
        commands::spaces::load_space(app, &open);
    }
}

/// Start the embed task. Opening the space already woke it, so its notes
/// are backfilled.
fn start_embedding(app: &AppHandle, embed_wake: Wake) {
    embed::sync::spawn(app.clone(), embed_wake.clone());
    // Off the launch, as a model moved to another drive is copied. The
    // legacy model is looked for once it has moved.
    let handle = app.clone();
    std::thread::spawn(move || {
        let state = handle.state::<AppState>();
        if embed::model::move_from_notes_root(&state.root, &state.local_data) {
            embed_wake.notify_one();
        }
        commands::models::replace_legacy_embedding_model(&handle);
    });
    startup::step("Starting the embedding task");
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
) -> Result<()> {
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

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.show();
        #[cfg(desktop)]
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// The tray menu's wording. The frontend holds the translations, so the
/// main window sends them once its language is known, and again on a change.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(mobile, allow(dead_code))]
struct TrayLabels {
    new_note: String,
    open: String,
    settings: String,
    quit: String,
}

/// Android has no tray to label.
#[cfg(mobile)]
#[tauri::command]
fn set_tray_labels(labels: TrayLabels) {
    let _ = labels;
}
