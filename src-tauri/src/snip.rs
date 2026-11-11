//! Region screenshots with annotations: the screen is frozen, shown in a full-screen overlay
//! where the user selects an area and draws on it, and the result is copied and/or saved.
use image::RgbaImage;

/// Decodes the PNG the overlay sends back, given as base64 with or without a data-URL prefix.
pub fn decode_png(data: &str) -> Result<RgbaImage, String> {
    let bytes = png_bytes(data)?;
    image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map(|img| img.to_rgba8())
        .map_err(|_| "Dữ liệu ảnh không phải PNG".to_string())
}

/// The raw bytes of a base64 picture, with or without a data-URL prefix.
pub fn png_bytes(data: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    let encoded = data.split_once("base64,").map_or(data, |(_, rest)| rest).trim();
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "Dữ liệu ảnh không hợp lệ".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn png_base64(image: &RgbaImage) -> String {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())
    }

    #[test]
    fn decodes_a_png_with_or_without_the_data_url_prefix() {
        let image = RgbaImage::from_fn(5, 3, |x, y| image::Rgba([x as u8 * 40, y as u8 * 80, 7, 255]));
        let plain = png_base64(&image);
        assert_eq!(decode_png(&plain).unwrap(), image);
        assert_eq!(decode_png(&format!("data:image/png;base64,{plain}")).unwrap(), image);
    }

    #[test]
    fn rejects_data_that_is_not_a_picture() {
        assert!(decode_png("").is_err());
        assert!(decode_png("khong phai base64 !!!").is_err());
        assert!(decode_png("aGVsbG8gd29ybGQ=").is_err()); // valid base64, not a PNG
    }
}

use crate::capture::{self, MonitorInfo, Target};
use crate::commands::AppState;
use crate::{events, screenshot, shell};
use hp_core::geometry::{Rect, monitor_containing};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

pub const SNIP: &str = "snip";

/// What the overlay needs: the frozen screen and the monitor it covers.
#[derive(Clone, Debug, Serialize)]
pub struct SnipContext {
    /// Image file of the frozen screen at physical resolution.
    pub frozen: String,
    pub monitor: MonitorInfo,
    /// Windows the selection can snap to, top-most first, as they were when the screen froze.
    pub windows: Vec<SnapWindow>,
}

/// A window on the frozen screen, relative to the monitor, in physical pixels.
#[derive(Clone, Debug, Serialize)]
pub struct SnapWindow {
    pub rect: Rect,
    pub title: String,
}

/// What to do with the finished picture.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnipAction {
    Copy,
    Save,
    Both,
}

#[derive(Default)]
pub struct SnipState(Mutex<Option<SnipContext>>);

fn frozen_file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("snip-frozen.bmp"))
}

/// Freezes the monitor under the mouse and opens the overlay on it. HP Screen's own windows
/// are excluded from capture, so they never appear in the frozen picture.
pub fn start(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window(SNIP).is_some() {
        return Ok(()); // already open
    }
    shell::suspend_bar(app);
    let opened = open_overlay(app);
    if opened.is_err() {
        shell::resume_bar(app);
    }
    opened
}

fn open_overlay(app: &AppHandle) -> Result<(), String> {
    let monitors = capture::list_monitors();
    let mut cursor = POINT::default();
    // SAFETY: the out pointer is valid.
    let _ = unsafe { GetCursorPos(&mut cursor) };
    let rects: Vec<_> = monitors.iter().map(|m| m.rect).collect();
    let index = monitor_containing((cursor.x, cursor.y), &rects).unwrap_or(0);
    let monitor = monitors.into_iter().nth(index).ok_or("Không tìm thấy màn hình nào")?;

    // Listed before the overlay exists; HP Screen's own windows are left out anyway.
    let windows = snap_windows(&monitor);
    let image = capture::grab_frame(&Target::Monitor { index: monitor.index })?;
    let path = frozen_file(app)?;
    // BMP: written quickly, which keeps the delay between the key press and the overlay short.
    image::DynamicImage::ImageRgba8(image)
        .to_rgb8()
        .save_with_format(&path, image::ImageFormat::Bmp)
        .map_err(|e| format!("Không lưu được ảnh tạm: {e}"))?;

    let context = SnipContext { frozen: path.to_string_lossy().into_owned(), monitor: monitor.clone(), windows };
    *app.state::<SnipState>().0.lock().unwrap_or_else(|e| e.into_inner()) = Some(context);

    let window = WebviewWindowBuilder::new(app, SNIP, WebviewUrl::App("index.html".into()))
        .title("HP Screen - Chụp ảnh")
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build()
        .map_err(|e| format!("Không mở được công cụ chụp ảnh: {e}"))?;
    let _ = window.set_position(PhysicalPosition::new(monitor.rect.x, monitor.rect.y));
    let _ = window.set_size(PhysicalSize::new(monitor.rect.w, monitor.rect.h));
    Ok(())
}

pub fn context(app: &AppHandle) -> Option<SnipContext> {
    app.state::<SnipState>().0.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Copies and/or saves the finished picture, then closes the overlay.
pub fn finish(app: &AppHandle, action: SnipAction, png: &str) -> Result<Option<String>, String> {
    let image = decode_png(png)?;
    let mut saved = None;
    if matches!(action, SnipAction::Save | SnipAction::Both) {
        let settings = app.state::<AppState>().settings();
        let dir = crate::paths::output_dir(app, &settings)?;
        let path = screenshot::save_png(&dir, &settings, &image)?;
        saved = Some(path.to_string_lossy().into_owned());
        events::library_changed(app);
    }
    if matches!(action, SnipAction::Copy | SnipAction::Both) {
        screenshot::copy_to_clipboard(&image)?;
    }
    close(app);
    Ok(saved)
}

pub fn close(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SNIP) {
        let _ = window.destroy();
    }
    app.state::<SnipState>().0.lock().unwrap_or_else(|e| e.into_inner()).take();
    shell::resume_bar(app);
    if let Ok(path) = frozen_file(app) {
        let _ = std::fs::remove_file(path);
    }
}

/// Windows on `monitor` the selection can snap to, top-most first.
///
/// The frame Windows draws (`DWMWA_EXTENDED_FRAME_BOUNDS`) is used rather than the window
/// rectangle, which also counts an invisible resize border of about 7 px on each side.
/// Minimized, hidden, cloaked (another virtual desktop, suspended apps), click-through and
/// untitled windows are skipped, as are the desktop and HP Screen's own windows.
fn snap_windows(monitor: &MonitorInfo) -> Vec<SnapWindow> {
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GWL_EXSTYLE, GWL_STYLE, GetClassNameW, GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
        GetWindowThreadProcessId, IsIconic, IsWindowVisible, WS_CHILD, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };
    use windows::core::BOOL;

    fn candidate(hwnd: HWND) -> Option<(Rect, String)> {
        // SAFETY: `hwnd` comes from EnumWindows; every out pointer is a valid local of the right size.
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
                return None;
            }
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            if style & WS_CHILD.0 != 0 || ex_style & (WS_EX_TOOLWINDOW.0 | WS_EX_TRANSPARENT.0) != 0 {
                return None;
            }
            let mut cloaked = 0u32;
            let size = std::mem::size_of::<u32>() as u32;
            if DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut _, size).is_ok() && cloaked != 0 {
                return None;
            }
            // The desktop itself covers every monitor; the overlay offers "whole screen" there instead.
            let mut class = [0u16; 32];
            let class_len = GetClassNameW(hwnd, &mut class).max(0) as usize;
            if matches!(String::from_utf16_lossy(&class[..class_len]).as_str(), "Progman" | "WorkerW") {
                return None;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == std::process::id() {
                return None;
            }
            let length = GetWindowTextLengthW(hwnd);
            if length <= 0 {
                return None;
            }
            let mut text = vec![0u16; length as usize + 1];
            let copied = GetWindowTextW(hwnd, &mut text).max(0) as usize;
            let title = String::from_utf16_lossy(&text[..copied]).trim().to_string();
            if title.is_empty() {
                return None;
            }
            let mut r = RECT::default();
            let size = std::mem::size_of::<RECT>() as u32;
            if DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut RECT as *mut _, size).is_err() {
                GetWindowRect(hwnd, &mut r).ok()?;
            }
            let rect = Rect { x: r.left, y: r.top, w: (r.right - r.left).max(0) as u32, h: (r.bottom - r.top).max(0) as u32 };
            Some((rect, title))
        }
    }

    unsafe extern "system" fn collect(hwnd: HWND, list: LPARAM) -> BOOL {
        // SAFETY: `list` is the vector passed to EnumWindows below, alive for the whole call.
        let list = unsafe { &mut *(list.0 as *mut Vec<(Rect, String)>) };
        if let Some(found) = candidate(hwnd) {
            list.push(found);
        }
        BOOL(1)
    }

    let mut found: Vec<(Rect, String)> = Vec::new();
    // SAFETY: the callback only touches `found`, which outlives the call.
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut found as *mut _ as isize)) };
    let rects: Vec<Rect> = found.iter().map(|(r, _)| *r).collect();
    hp_core::snip::snap_targets(&rects, monitor.rect)
        .into_iter()
        .map(|(i, rect)| SnapWindow { rect, title: found[i].1.clone() })
        .collect()
}

#[cfg(test)]
mod desktop_tests {
    //! Needs a real desktop: `cargo test -p hp-screen --lib -- --ignored snap`
    use super::*;

    #[test]
    #[ignore = "needs a desktop session"]
    fn snap_windows_lists_visible_windows_inside_the_monitor() {
        let monitor = capture::list_monitors().into_iter().find(|m| m.primary).expect("a primary monitor");
        let windows = snap_windows(&monitor);
        for w in &windows {
            println!("{:?} {}", w.rect, w.title);
        }
        assert!(windows.iter().all(|w| w.rect.x >= 0 && w.rect.y >= 0));
        assert!(windows.iter().all(|w| w.rect.x as u32 + w.rect.w <= monitor.rect.w && w.rect.y as u32 + w.rect.h <= monitor.rect.h));
    }
}
