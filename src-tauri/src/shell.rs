//! The app's presence on the desktop: tray icon, closing to the tray, and the compact bar.
//!
//! HP Screen's own windows are excluded from every capture, so the app never shows up in its
//! own videos or screenshots even when it sits inside the recorded area.
//!
//! The compact bar is one small always-on-top row that can be dragged anywhere. It holds the
//! recording and screenshot controls, so the app can be used without the main window. It is
//! shown while recording, and also whenever the user switches to "compact mode".
use crate::commands::AppState;
use crate::hotkeys::{self, HotkeyAction};
use hp_core::state::RecState;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::Duration;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow, WindowEvent};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};

pub const MAIN: &str = "main";
pub const CONTROL_BAR: &str = "control-bar";
const BAR_MARGIN_BOTTOM: i32 = 96;

/// The user chose the compact bar instead of the main window.
static COMPACT: AtomicBool = AtomicBool::new(false);
/// The bar was placed once; after that it stays wherever the user drags it.
static BAR_PLACED: AtomicBool = AtomicBool::new(false);
/// The main window was on screen when a recording started, so it comes back afterwards.
static MAIN_WAS_VISIBLE: AtomicBool = AtomicBool::new(false);
/// The bar was hidden for an overlay (area selection, capture-and-annotate) and comes back after.
static BAR_SUSPENDED: AtomicBool = AtomicBool::new(false);

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItemBuilder::with_id("toggle", "Bắt đầu / Dừng quay").build(app)?;
    let shot = MenuItemBuilder::with_id("screenshot", "Chụp ảnh và vẽ ghi chú").build(app)?;
    let quick = MenuItemBuilder::with_id("quickshot", "Chụp nhanh").build(app)?;
    let compact = MenuItemBuilder::with_id("compact", "Thanh gọn").build(app)?;
    let open = MenuItemBuilder::with_id("open", "Mở HP Screen").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Thoát").build(app)?;
    let menu = MenuBuilder::new(app)
        .items(&[&toggle, &shot, &quick])
        .separator()
        .items(&[&compact, &open])
        .separator()
        .items(&[&quit])
        .build()?;

    let mut tray = TrayIconBuilder::with_id("hp-tray").tooltip("HP Screen").menu(&menu).show_menu_on_left_click(false);
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.on_menu_event(|app, event| match event.id().as_ref() {
        "toggle" => hotkeys::run_action(app, HotkeyAction::Toggle),
        "screenshot" => hotkeys::run_action(app, HotkeyAction::Screenshot),
        "quickshot" => hotkeys::run_action(app, HotkeyAction::Quickshot),
        "compact" => show_compact(app),
        "open" => show_main(app),
        "quit" => quit_app(app),
        _ => {}
    })
    .on_tray_icon_event(|tray, event| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
            let app = tray.app_handle();
            if COMPACT.load(Ordering::SeqCst) { show_compact(app) } else { show_main(app) }
        }
    })
    .build(app)?;

    if let Some(main) = app.get_webview_window(MAIN) {
        exclude_from_capture(&main);
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
    #[cfg(debug_assertions)]
    if std::env::var_os("HP_DEV_START_COMPACT").is_some() {
        show_compact(app);
    }
    Ok(())
}

/// Keeps a window out of every capture: it stays visible on screen but never in a recording
/// or screenshot.
pub fn exclude_from_capture(window: &WebviewWindow) {
    // Development only: lets the author screenshot the UI to check it.
    #[cfg(debug_assertions)]
    if std::env::var_os("HP_DEV_ALLOW_CAPTURE").is_some() {
        return;
    }
    if let Ok(hwnd) = window.hwnd() {
        // SAFETY: the handle belongs to a live window of this process.
        unsafe {
            let _ = SetWindowDisplayAffinity(HWND(hwnd.0), WDA_EXCLUDEFROMCAPTURE);
        }
    }
}

pub fn show_main(app: &AppHandle) {
    COMPACT.store(false, Ordering::SeqCst);
    if !is_session_active(app) {
        if let Some(bar) = app.get_webview_window(CONTROL_BAR) {
            let _ = bar.hide();
        }
    }
    if let Some(main) = app.get_webview_window(MAIN) {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
}

/// Switches to the compact bar: the main window goes away, the bar appears.
pub fn show_compact(app: &AppHandle) {
    COMPACT.store(true, Ordering::SeqCst);
    if let Some(main) = app.get_webview_window(MAIN) {
        let _ = main.hide();
    }
    show_bar(app);
}

/// Hides the bar (the app stays in the tray). During a recording the bar stays.
pub fn hide_compact(app: &AppHandle) {
    COMPACT.store(false, Ordering::SeqCst);
    if !is_session_active(app) {
        if let Some(bar) = app.get_webview_window(CONTROL_BAR) {
            let _ = bar.hide();
        }
    }
}

fn show_bar(app: &AppHandle) {
    if let Some(bar) = app.get_webview_window(CONTROL_BAR) {
        if !BAR_PLACED.swap(true, Ordering::SeqCst) {
            place_bar(&bar);
        }
        let _ = bar.show();
    }
}

fn is_session_active(app: &AppHandle) -> bool {
    app.try_state::<AppState>().is_some_and(|s| s.recorder.status().state != RecState::Idle)
}

/// Hides the bar while a full-screen overlay is open, so it does not sit on top of it.
pub fn suspend_bar(app: &AppHandle) {
    if let Some(bar) = app.get_webview_window(CONTROL_BAR) {
        if bar.is_visible().unwrap_or(false) {
            BAR_SUSPENDED.store(true, Ordering::SeqCst);
            let _ = bar.hide();
        }
    }
}

/// Brings the bar back after an overlay closes, if it was hidden for it.
pub fn resume_bar(app: &AppHandle) {
    if BAR_SUSPENDED.swap(false, Ordering::SeqCst) {
        show_bar(app);
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

/// While a session is active the main window steps aside and the bar floats on top; when it
/// ends, whatever was on screen before comes back. Called on every state report; acts only
/// on changes.
pub fn sync_windows(app: &AppHandle, state: RecState) {
    static LAST: AtomicU8 = AtomicU8::new(0);
    let active = !matches!(state, RecState::Idle);
    if LAST.swap(active as u8, Ordering::SeqCst) == active as u8 {
        return;
    }
    let main = app.get_webview_window(MAIN);
    if active {
        if let Some(main) = &main {
            let visible = main.is_visible().unwrap_or(false) && !main.is_minimized().unwrap_or(false);
            MAIN_WAS_VISIBLE.store(visible, Ordering::SeqCst);
            let _ = main.hide();
        }
        show_bar(app);
    } else {
        if !COMPACT.load(Ordering::SeqCst) {
            if let Some(bar) = app.get_webview_window(CONTROL_BAR) {
                let _ = bar.hide();
            }
        }
        if MAIN_WAS_VISIBLE.swap(false, Ordering::SeqCst) {
            if let Some(main) = &main {
                let _ = main.show();
                let _ = main.unminimize();
            }
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
