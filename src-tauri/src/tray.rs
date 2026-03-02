use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

use crate::settings;
use crate::state::AppState;

/// Managed state holding the tray icon ID so we can update it later.
pub struct TrayIconState(pub Mutex<Option<tauri::tray::TrayIconId>>);

// Embedded template icons (black on transparent, macOS will invert as needed)
const ICON_IDLE: &[u8] = include_bytes!("icons/tray-idle@2x.png");
const ICON_RECORDING: &[u8] = include_bytes!("icons/tray-recording@2x.png");
const ICON_ERROR: &[u8] = include_bytes!("icons/tray-error@2x.png");

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let current_settings = settings::load(app);
    let show_item = MenuItem::with_id(app, "show", "Show Calliope", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let postprocess_item = CheckMenuItem::with_id(
        app,
        "postprocess",
        "Post-processing",
        true,
        current_settings.postprocess_enabled,
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&show_item, &settings_item, &separator, &postprocess_item, &quit_item],
    )?;

    let tray = TrayIconBuilder::new()
        .icon(Image::from_bytes(ICON_IDLE).expect("failed to load tray idle icon"))
        .icon_as_template(cfg!(target_os = "macos"))
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                show_main(app);
            }
            "settings" => {
                show_settings(app);
            }
            "postprocess" => {
                toggle_postprocess(app);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_popover(tray.app_handle());
            }
        })
        .build(app)?;

    // Store the tray icon ID in managed state
    if let Some(state) = app.try_state::<TrayIconState>() {
        *state.0.lock().unwrap() = Some(tray.id().clone());
    }

    Ok(())
}

/// Update the tray icon to reflect the current app state.
pub fn update_icon(app: &AppHandle, state: &AppState) {
    let icon_bytes = match state {
        AppState::Recording => ICON_RECORDING,
        AppState::Error(_) => ICON_ERROR,
        _ => ICON_IDLE,
    };

    let tray_state = app.state::<TrayIconState>();
    let guard = tray_state.0.lock().unwrap();
    if let Some(ref id) = *guard {
        if let Some(tray) = app.tray_by_id(id) {
            if let Ok(icon) = Image::from_bytes(icon_bytes) {
                let _ = tray.set_icon(Some(icon));
                #[cfg(target_os = "macos")]
                let _ = tray.set_icon_as_template(true);
            }
        }
    }
}

fn toggle_popover(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            position_popover_near_tray(&win);
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

fn position_popover_near_tray(win: &tauri::WebviewWindow) {
    // Position near tray: top-right on macOS, bottom-right on others
    if let Ok(Some(monitor)) = win.primary_monitor() {
        let screen = monitor.size();
        let scale = monitor.scale_factor();
        let win_size = win.outer_size().unwrap_or(tauri::PhysicalSize {
            width: 280,
            height: 420,
        });

        #[cfg(target_os = "macos")]
        {
            // macOS: tray is at top of screen, position below menu bar near right edge
            let x = (screen.width as f64 / scale) as i32 - win_size.width as i32 - 8;
            let y = 30; // below macOS menu bar
            let _ = win.set_position(tauri::PhysicalPosition::new(
                (x as f64 * scale) as i32,
                (y as f64 * scale) as i32,
            ));
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Windows/Linux: tray is typically at bottom, position above taskbar near right edge
            let x = (screen.width as f64 / scale) as i32 - win_size.width as i32 - 8;
            let y = (screen.height as f64 / scale) as i32 - win_size.height as i32 - 48;
            let _ = win.set_position(tauri::PhysicalPosition::new(
                (x as f64 * scale) as i32,
                (y as f64 * scale) as i32,
            ));
        }
    }
}

fn show_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        position_popover_near_tray(&win);
        let _ = win.show();
        let _ = win.set_focus();
    }
}

fn show_settings(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

fn toggle_postprocess(app: &AppHandle) {
    let mut s = settings::load(app);
    s.postprocess_enabled = !s.postprocess_enabled;
    if let Err(e) = settings::save(app, &s) {
        log::error!("Failed to save post-processing toggle: {e}");
    }
}
