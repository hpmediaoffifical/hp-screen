//! The app's presence on the desktop: tray icon, closing to the tray, and the small control
//! bar shown while recording (hidden from the recording itself).
use crate::commands::AppState;
use crate::hotkeys::{self, HotkeyAction};
use hp_core::state::RecState;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow, WindowEvent};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};

pub const MAIN: &str = "main";
pub const CONTROL_BAR: &str = "control-bar";
const BAR_MARGIN_BOTTOM: i32 = 96;

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItemBuilder::with_id("toggle", "Bắt đầu / Dừng quay").build(app)?;
    let shot = MenuItemBuilder::with_id("screenshot", "Chụp ảnh").build(app)?;
    let open = MenuItemBuilder::with_id("open", "Mở HP Screen").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Thoát").build(app)?;
    let menu = MenuBuilder::new(app).items(&[&toggle, &shot]).separator().items(&[&open]).separator().items(&[&quit]).build()?;

    let mut tray = TrayIconBuilder::with_id("hp-tray").tooltip("HP Screen").menu(&menu).show_menu_on_left_click(false);
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.on_menu_event(|app, event| match event.id().as_ref() {
        "toggle" => hotkeys::run_action(app, HotkeyAction::Toggle),
        "screenshot" => hotkeys::run_action(app, HotkeyAction::Screenshot),
        "open" => show_main(app),
        "quit" => quit_app(app),
        _ => {}
    })
    .on_tray_icon_event(|tray, event| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
            show_main(tray.app_handle());
        }
    })
    .build(app)?;

    if let Some(main) = app.get_webview_window(MAIN) {
        let window = main.clone();
        main.on_window_event(move |event| {
            // Closing the window keeps the app (and its hotkeys) alive in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        });
    }
    if let Some(bar) = app.get_webview_window(CONTROL_BAR) {
        exclude_from_capture(&bar);
    }
    Ok(())
}

/// Keeps a window out of every capture, so the control bar never appears in a recording.
fn exclude_from_capture(window: &WebviewWindow) {
    if let Ok(hwnd) = window.hwnd() {
        // SAFETY: the handle belongs to a live window of this process.
        unsafe {
            let _ = SetWindowDisplayAffinity(HWND(hwnd.0), WDA_EXCLUDEFROMCAPTURE);
        }
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(main) = app.get_webview_window(MAIN) {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
}

/// Stops a running recording so the file is closed properly, then exits.
fn quit_app(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Some(state) = app.try_state::<AppState>() {
            let _ = state.recorder.cancel();
            let _ = state.recorder.stop();
            for _ in 0..100 {
                if state.recorder.status().state == RecState::Idle {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        app.exit(0);
    });
}

/// Shows the control bar and gets the main window out of the way while a session is active;
/// restores both when it ends. Called on every state report; acts only on changes.
pub fn sync_windows(app: &AppHandle, state: RecState) {
    static LAST: AtomicU8 = AtomicU8::new(0);
    let active = !matches!(state, RecState::Idle);
    if LAST.swap(active as u8, Ordering::SeqCst) == active as u8 {
        return;
    }
    let bar = app.get_webview_window(CONTROL_BAR);
    let main = app.get_webview_window(MAIN);
    if active {
        if let Some(main) = &main {
            let _ = main.minimize();
        }
        if let Some(bar) = &bar {
            place_bar(bar);
            let _ = bar.show();
        }
    } else {
        if let Some(bar) = &bar {
            let _ = bar.hide();
        }
        if let Some(main) = &main {
            let _ = main.unminimize();
            let _ = main.show();
        }
    }
}

/// Bottom centre of the primary monitor.
fn place_bar(bar: &WebviewWindow) {
    let (Ok(Some(monitor)), Ok(size)) = (bar.primary_monitor(), bar.outer_size()) else { return };
    let (area, origin) = (monitor.size(), monitor.position());
    let x = origin.x + (area.width as i32 - size.width as i32) / 2;
    let y = origin.y + area.height as i32 - size.height as i32 - BAR_MARGIN_BOTTOM;
    let _ = bar.set_position(PhysicalPosition::new(x, y));
}
