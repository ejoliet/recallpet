//! Tray icon + the borderless popover window it toggles.
//!
//! The popover window is declared once in `tauri.conf.json` and only ever
//! shown/hidden here - it is never destroyed and recreated, so its webview
//! state (loaded items, scroll position, search text) survives every
//! open/close cycle.

use std::time::Duration;

use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime, WindowEvent,
};
use tauri_plugin_positioner::{Position, WindowExt};

const IDLE_ICON: &[u8] = include_bytes!("../icons/tray-idle.png");
const CAPTURE_ICON: &[u8] = include_bytes!("../icons/tray-capture.png");
const PAUSED_ICON: &[u8] = include_bytes!("../icons/tray-paused.png");

/// How long the tray icon flashes the "capture acknowledged" frame before
/// reverting. Event-driven, one-shot - not a repeating animation loop.
const CAPTURE_BLINK: Duration = Duration::from_millis(450);

pub const TRAY_ID: &str = "recallpet-tray";
pub const POPOVER_LABEL: &str = "popover";
const QUIT_MENU_ID: &str = "quit";

/// Builds the tray icon (left-click toggles the popover, right-click shows a
/// minimal Quit menu - Phase 0 has no other menu/settings surface) and wires
/// the popover's focus-loss auto-hide. Called once from the app's `setup`
/// hook.
pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let idle_icon =
        Image::from_bytes(IDLE_ICON).expect("bundled tray-idle.png must be a valid PNG");
    let quit_item = MenuItem::with_id(app, QUIT_MENU_ID, "Quit RecallPet", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&quit_item])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(idle_icon)
        .icon_as_template(true)
        .tooltip("RecallPet")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if event.id().as_ref() == QUIT_MENU_ID {
                app.exit(0);
            }
        })
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();
            tauri_plugin_positioner::on_tray_event(app, &event);

            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_popover(app);
            }
        })
        .build(app)?;

    if let Some(window) = app.get_webview_window(POPOVER_LABEL) {
        let window_to_hide = window.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::Focused(false) = event {
                let _ = window_to_hide.hide();
            }
        });
    }

    Ok(())
}

fn toggle_popover<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(POPOVER_LABEL) else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    let _ = window.move_window_constrained(Position::TrayBottomCenter);
    let _ = window.show();
    let _ = window.set_focus();
}

/// Swaps the tray icon between the idle/paused frames and tells the popover
/// to update its avatar. Called from `set_collection_paused`.
pub fn set_paused_visual<R: Runtime>(app: &AppHandle<R>, paused: bool) {
    let bytes = if paused { PAUSED_ICON } else { IDLE_ICON };
    apply_icon(app, bytes);
    let _ = app.emit(
        "avatar:state-changed",
        if paused { "paused" } else { "idle" },
    );
}

/// Briefly flashes the "capture acknowledged" tray frame, then reverts to
/// idle or paused (whichever the collector actually is once the blink ends).
/// One-shot timer, not a recurring loop.
pub fn signal_capture<R: Runtime + 'static>(app: &AppHandle<R>, currently_paused: bool) {
    apply_icon(app, CAPTURE_ICON);
    let _ = app.emit("avatar:state-changed", "capture");

    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(CAPTURE_BLINK);
        set_paused_visual(&app, currently_paused);
    });
}

fn apply_icon<R: Runtime>(app: &AppHandle<R>, bytes: &[u8]) {
    let Ok(icon) = Image::from_bytes(bytes) else {
        log::error!("failed to decode a bundled tray icon frame");
        return;
    };
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(icon));
    }
}
