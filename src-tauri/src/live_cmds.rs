//! Commands and windows for what is drawn live into a recording: the on-screen drawing layer
//! (and, later, the webcam bubble). Both windows are excluded from capture; what reaches the
//! video is drawn by `LiveLayer` in the pipeline.
use crate::capture;
use crate::commands::AppState;
use crate::shell::exclude_from_capture;
use hp_core::geometry::Rect;
use hp_core::state::RecState;
use hp_core::stroke::Stroke;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindowBuilder};

pub const DRAW_WINDOW: &str = "draw";
pub const DRAW_MODE: &str = "draw-mode";

#[derive(Clone, Serialize)]
pub struct DrawContext {
    pub on: bool,
    pub hold_ms: i64,
}

#[derive(Clone, Serialize)]
struct DrawMode {
    on: bool,
}

/// The drawing window covers exactly the recorded area (its even-sized part).
fn overlay_rect(target: Rect) -> Rect {
    Rect { w: target.w & !1, h: target.h & !1, ..target }
}

fn recording(state: &AppState) -> bool {
    matches!(state.recorder.status().state, RecState::Recording | RecState::Paused)
}

/// Turns drawing on or off. On: the drawing window covers the recorded area and takes the
/// mouse. Off: the mouse goes through it again, while finished strokes fade on screen.
pub async fn set_drawing(app: AppHandle, on: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    if on && !recording(&state) {
        return Err("Chỉ vẽ được khi đang quay".to_string());
    }
    let live = state.recorder.live();
    let window = match app.get_webview_window(DRAW_WINDOW) {
        Some(w) => w,
        None if !on => return Ok(()),
        None => {
            let target = state.recorder.last_target().ok_or("Chưa chọn vùng quay")?;
            let area = overlay_rect(capture::target_rect(&target)?);
            let w = WebviewWindowBuilder::new(&app, DRAW_WINDOW, WebviewUrl::App("index.html".into()))
                .title("HP Screen - Vẽ")
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .visible(false)
                .build()
                .map_err(|e| format!("Không mở được lớp vẽ: {e}"))?;
            let _ = w.set_position(PhysicalPosition::new(area.x, area.y));
            let _ = w.set_size(PhysicalSize::new(area.w, area.h));
            exclude_from_capture(&w);
            let _ = w.show();
            w
        }
    };
    live.set_drawing(on);
    let _ = window.set_ignore_cursor_events(!on);
    if on {
        let _ = window.set_focus();
    }
    let _ = app.emit(DRAW_MODE, DrawMode { on });
    Ok(())
}

/// The hotkey and the bar button.
pub async fn toggle_drawing(app: AppHandle) -> Result<(), String> {
    let on = app.state::<AppState>().recorder.live().is_drawing();
    set_drawing(app, !on).await
}

/// Closes the live windows when a recording ends.
pub fn close_windows(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(DRAW_WINDOW) {
        let _ = w.close();
    }
    let _ = app.emit(DRAW_MODE, DrawMode { on: false });
}

#[tauri::command]
pub async fn draw_set(app: AppHandle, on: bool) -> Result<(), String> {
    set_drawing(app, on).await
}

#[tauri::command]
pub async fn draw_toggle(app: AppHandle) -> Result<(), String> {
    toggle_drawing(app).await
}

#[tauri::command]
pub fn draw_stroke(state: State<'_, AppState>, stroke: Stroke) {
    state.recorder.live().upsert_stroke(stroke);
}

#[tauri::command]
pub fn draw_clear(state: State<'_, AppState>) {
    state.recorder.live().clear();
}

#[tauri::command]
pub fn draw_context(state: State<'_, AppState>) -> DrawContext {
    let live = state.recorder.live();
    DrawContext { on: live.is_drawing(), hold_ms: state.settings().draw_fade_secs as i64 * 1000 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_window_rect_matches_the_target() {
        assert_eq!(overlay_rect(Rect { x: -1920, y: 10, w: 1921, h: 1081 }), Rect { x: -1920, y: 10, w: 1920, h: 1080 });
        assert_eq!(overlay_rect(Rect { x: 0, y: 0, w: 2560, h: 1600 }), Rect { x: 0, y: 0, w: 2560, h: 1600 });
    }
}
