//! What only desktops have: the tray, the capture window and its global
//! hotkey, launching again, launch at login and self-update. Android has
//! none of them.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{
    AppHandle, Builder, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent, Wry,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::{show_main, startup, Result, TrayLabels, CAPTURE, MAIN};

/// Toggles the capture window, for desktops where apps cannot register a
/// global shortcut (Wayland): the user binds this command there instead.
const CAPTURE_FLAG: &str = "--capture";

/// The desktop plugins.
pub fn plugins(builder: Builder<Wry>) -> Builder<Wry> {
    builder
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
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_capture(app);
                    }
                })
                .build(),
        )
}

/// Make the capture window and the tray, and register the capture hotkey.
/// Launched with `--capture`, the capture window shows at once.
pub fn setup(app: &AppHandle, hotkey: &str) -> tauri::Result<()> {
    build_capture_window(app)?;
    startup::step("Making the capture window");
    build_tray(app)?;
    keep_main_window_alive(app);
    startup::step("Making the tray icon");

    if let Err(e) = app.global_shortcut().register(hotkey) {
        log::error!("could not register the capture hotkey {hotkey}: {e}");
    }
    if std::env::args().any(|a| a == CAPTURE_FLAG) {
        toggle_capture(app);
    }
    startup::step("Registering the capture shortcut");
    Ok(())
}

/// The capture window is built once at startup and only ever shown and
/// hidden, so the hotkey never pays for window creation. It opens centred;
/// where the user moves and sizes it then holds until the app quits.
fn build_capture_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, CAPTURE, WebviewUrl::App("capture/".into()))
        .title("Scratchnote capture")
        .inner_size(620.0, 200.0)
        // At 16px text; the webview grows it with the text size setting.
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

fn tray_menu(app: &AppHandle, labels: &TrayLabels) -> tauri::Result<Menu<Wry>> {
    let new_note = MenuItem::with_id(app, "new_note", &labels.new_note, true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", &labels.open, true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", &labels.settings, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", &labels.quit, true, None::<&str>)?;
    Menu::with_items(app, &[&new_note, &open, &settings, &quit])
}

#[tauri::command]
pub fn set_tray_labels(app: AppHandle, labels: TrayLabels) -> Result<()> {
    let tray = app.tray_by_id("tray").ok_or("no tray icon")?;
    let menu = tray_menu(&app, &labels)?;
    Ok(tray.set_menu(Some(menu))?)
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = tray_menu(app, &TrayLabels::default())?;
    let icon = app
        .default_window_icon()
        .ok_or_else(|| tauri::Error::AssetNotFound("the window icon".into()))?;

    TrayIconBuilder::with_id("tray")
        .icon(icon.clone())
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
