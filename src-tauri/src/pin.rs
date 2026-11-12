//! Pinned pictures: a finished capture kept on screen in its own small borderless window that
//! stays on top. Each pin can be dragged, resized, copied and closed; several can be open.
//! Like every HP Screen window, pins never appear in captures.
use crate::{screenshot, shell, snip};
use hp_core::geometry::Rect;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex};
use tauri::{AppHandle, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

pub const PREFIX: &str = "pin-";
/// A pin never opens larger than this share of its monitor.
const MAX_SHARE: f64 = 0.9;

/// PNG bytes of every open pin, by window label.
static PINS: LazyLock<Mutex<HashMap<String, Vec<u8>>>> = LazyLock::new(Default::default);
static NEXT: AtomicU32 = AtomicU32::new(1);

fn pins() -> std::sync::MutexGuard<'static, HashMap<String, Vec<u8>>> {
    PINS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Size a `width` × `height` picture opens at on a `screen_w` × `screen_h` monitor: its own
/// size (so it sits exactly over what was captured) unless that would not fit.
pub fn fit(width: u32, height: u32, screen_w: u32, screen_h: u32) -> (u32, u32) {
    let (w, h) = (width.max(1) as f64, height.max(1) as f64);
    let k = (screen_w as f64 * MAX_SHARE / w).min(screen_h as f64 * MAX_SHARE / h).min(1.0);
    (((w * k).round() as u32).max(1), ((h * k).round() as u32).max(1))
}

/// Pins the overlay's picture where its area was (`rect`, monitor-relative physical pixels)
/// and closes the overlay.
pub fn open(app: &AppHandle, png: &str, rect: Rect) -> Result<(), String> {
    let ctx = snip::context(app).ok_or("Công cụ chụp ảnh chưa được mở")?;
    let bytes = snip::png_bytes(png)?;
    let (width, height) =
        image::ImageReader::with_format(std::io::Cursor::new(&bytes), image::ImageFormat::Png)
            .into_dimensions()
            .map_err(|_| "Dữ liệu ảnh không phải PNG".to_string())?;
    let m = ctx.monitor.rect;
    let (w, h) = fit(width, height, m.w, m.h);
    // Where the area was, moved back inside the monitor if the picture had to shrink.
    let x = (m.x + rect.x).clamp(m.x, m.x + m.w as i32 - w as i32);
    let y = (m.y + rect.y).clamp(m.y, m.y + m.h as i32 - h as i32);

    let label = format!("{PREFIX}{}", NEXT.fetch_add(1, Ordering::SeqCst));
    pins().insert(label.clone(), bytes);
    let built = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title("HP Screen - Ảnh ghim")
        .decorations(false)
        .shadow(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(true)
        // Double-clicking a pin resets its size; it must never maximize instead.
        .maximizable(false)
        .visible(false)
        .build();
    let window = match built {
        Ok(window) => window,
        Err(e) => {
            pins().remove(&label);
            return Err(format!("Không ghim được ảnh: {e}"));
        }
    };
    shell::exclude_from_capture(&window);
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.set_size(PhysicalSize::new(w, h));
    let closing = label.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Destroyed = event {
            pins().remove(&closing);
        }
    });
    snip::close(app);
    Ok(())
}

/// The pin's picture (PNG), for the window to show.
pub fn image(window: &WebviewWindow) -> Result<Vec<u8>, String> {
    pins().get(window.label()).cloned().ok_or_else(|| "Ảnh ghim không còn nữa".to_string())
}

pub fn copy(window: &WebviewWindow) -> Result<(), String> {
    let bytes = image(window)?;
    let picture = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|_| "Ảnh ghim bị hỏng".to_string())?
        .to_rgba8();
    screenshot::copy_to_clipboard(&picture)
}

pub fn close(window: &WebviewWindow) {
    pins().remove(window.label());
    let _ = window.destroy();
}

/// The pin is drawn: show it (it was created hidden so it never flashes empty).
pub fn ready(window: &WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pin_opens_at_the_size_of_its_picture() {
        assert_eq!(fit(640, 360, 1920, 1080), (640, 360));
    }

    #[test]
    fn a_pin_too_big_for_the_screen_shrinks_keeping_its_shape() {
        assert_eq!(fit(1920, 1080, 1920, 1080), (1728, 972));
        assert_eq!(fit(400, 3000, 1920, 1080), (130, 972));
    }

    #[test]
    fn empty_pictures_still_get_a_window() {
        assert_eq!(fit(0, 0, 1920, 1080), (1, 1));
    }
}
