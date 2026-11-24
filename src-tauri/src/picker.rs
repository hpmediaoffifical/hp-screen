/*! © 2026 HP Media (hpvn.media) — HP Screen. Bảo lưu mọi quyền. HP không có đại lý khác, không hợp tác với đơn vị nào, mọi sao chép đều là vi phạm. */
//! The selection overlay: a transparent window over one monitor where the user drags out a
//! region or clicks a window. `pick` opens it and resolves when the overlay reports back.
use crate::capture::{self, MonitorInfo, Target, WindowInfo};
use crate::shell;
use hp_core::geometry::monitor_containing;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

pub const PICKER: &str = "picker";

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PickMode {
    Region,
    Window,
}

/// What the overlay needs to draw itself.
#[derive(Clone, Debug, Serialize)]
pub struct PickerContext {
    pub mode: PickMode,
    pub monitor: MonitorInfo,
    /// Top-most first.
    pub windows: Vec<WindowInfo>,
}

struct Session {
    context: PickerContext,
    /// The monitor as it looked when the overlay opened (BMP), for the overlay's magnifier.
    /// Region mode only; handed over once.
    frozen: Option<Vec<u8>>,
    done: Sender<Option<Target>>,
}

/// The overlay session in progress, if any.
#[derive(Default)]
pub struct PickerState(Mutex<Option<Session>>);

impl PickerState {
    fn take(&self) -> Option<Session> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

/// Opens the overlay on the monitor under the mouse and waits for the user's choice.
/// `Ok(None)` means the user cancelled.
pub async fn pick(app: AppHandle, mode: PickMode) -> Result<Option<Target>, String> {
    let monitors = capture::list_monitors();
    let mut cursor = POINT::default();
    // SAFETY: the out pointer is valid.
    let _ = unsafe { GetCursorPos(&mut cursor) };
    let rects: Vec<_> = monitors.iter().map(|m| m.rect).collect();
    let index = monitor_containing((cursor.x, cursor.y), &rects).unwrap_or(0);
    let monitor = monitors.into_iter().nth(index).ok_or("Không tìm thấy màn hình nào")?;

    if let Some(existing) = app.get_webview_window(PICKER) {
        let _ = existing.destroy();
    }
    // Get HP Screen's own windows out of the way so the desktop is visible; they come back
    // as they were afterwards.
    let main = app.get_webview_window(shell::MAIN);
    let main_was_visible = main
        .as_ref()
        .is_some_and(|m| m.is_visible().unwrap_or(false) && !m.is_minimized().unwrap_or(false));
    if let Some(main) = &main {
        let _ = main.hide();
    }
    shell::suspend_bar(&app);

    // Taken before the overlay exists, so its dimming is not in the picture. The magnifier is
    // an extra: without the picture the overlay works as before.
    let frozen = if mode == PickMode::Region {
        let index = monitor.index;
        tauri::async_runtime::spawn_blocking(move || frozen_bmp(index)).await.ok().flatten()
    } else {
        None
    };

    let (tx, rx) = mpsc::channel();
    let state = app.state::<PickerState>();
    let context = PickerContext { mode, monitor: monitor.clone(), windows: capture::list_windows() };
    *state.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(Session { context, frozen, done: tx });

    let window = WebviewWindowBuilder::new(&app, PICKER, WebviewUrl::App("index.html".into()))
        .title("HP Screen - Chọn vùng quay")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build()
        .map_err(|e| format!("Không mở được lớp chọn vùng: {e}"))?;
    let _ = window.set_position(PhysicalPosition::new(monitor.rect.x, monitor.rect.y));
    let _ = window.set_size(PhysicalSize::new(monitor.rect.w, monitor.rect.h));
    let _ = window.show();
    let _ = window.set_focus();
    {
        let app = app.clone();
        window.on_window_event(move |event| {
            // Closed without an answer (Alt+F4): dropping the sender resolves the pick as cancelled.
            if let WindowEvent::Destroyed = event {
                app.state::<PickerState>().take();
            }
        });
    }

    let choice = tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten())
        .await
        .map_err(|e| e.to_string())?;
    shell::resume_bar(&app);
    if main_was_visible {
        if let Some(main) = &main {
            let _ = main.show();
            let _ = main.set_focus();
        }
    }
    Ok(choice)
}

/// One frame of monitor `index` as BMP bytes (quick to write, and the webview reads it).
fn frozen_bmp(index: usize) -> Option<Vec<u8>> {
    let frame = capture::grab_frame(&Target::Monitor { index }).ok()?;
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(frame).to_rgb8().write_to(&mut bytes, image::ImageFormat::Bmp).ok()?;
    Some(bytes.into_inner())
}

/// The frozen monitor picture for the overlay's magnifier. Given out once.
pub fn frozen(app: &AppHandle) -> Option<Vec<u8>> {
    let state = app.state::<PickerState>();
    let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_mut().and_then(|s| s.frozen.take())
}

/// Called by the overlay when it loads.
pub fn context(app: &AppHandle) -> Option<PickerContext> {
    let state = app.state::<PickerState>();
    let guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().map(|s| s.context.clone())
}

/// Called by the overlay with the user's choice (`None` = cancelled). Closes the overlay.
pub fn finish(app: &AppHandle, target: Option<Target>) {
    if let Some(session) = app.state::<PickerState>().take() {
        let _ = session.done.send(target);
    }
    if let Some(window) = app.get_webview_window(PICKER) {
        let _ = window.destroy();
    }
}
