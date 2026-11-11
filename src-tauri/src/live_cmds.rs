//! Commands and windows for what is drawn live into a recording: the on-screen drawing layer
//! (and, later, the webcam bubble). Both windows are excluded from capture; what reaches the
//! video is drawn by `LiveLayer` in the pipeline.
use crate::camera::{Camera, CameraInfo};
use crate::capture::{self, Target};
use crate::commands::AppState;
use crate::shell::exclude_from_capture;
use hp_core::geometry::Rect;
use hp_core::state::RecState;
use hp_core::stroke::Stroke;
use hp_core::webcam::{WebcamSettings, bubble_rect, camera_crop};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
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

/// The bubble window on screen: the bubble's place in the frame, moved to the recorded area.
fn bubble_on_screen(area: Rect, bubble: Rect) -> Rect {
    Rect { x: area.x + bubble.x, y: area.y + bubble.y, ..bubble }
}

/// Whether the webcam turns itself on: once per recording, when enabled in the settings.
fn should_auto_start(state: RecState, enabled: bool, attempted: bool) -> bool {
    state == RecState::Recording && enabled && !attempted
}

/// Voids a slow start (the camera takes up to seconds to open) when a stop or a newer start
/// happens meanwhile.
#[derive(Default)]
pub struct StartGate {
    generation: std::sync::atomic::AtomicU64,
}

impl StartGate {
    /// A ticket for a start that is beginning now; it voids any earlier one.
    pub fn begin(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }
    /// A stop: every start still under way is void.
    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
    pub fn still_valid(&self, ticket: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == ticket
    }
}

/// Height of the bubble's preview picture.
const PREVIEW_HEIGHT: u32 = 240;
const PREVIEW_EVERY: Duration = Duration::from_millis(100);

/// The bubble's picture: the cropped (and mirrored) camera frame, 240 px tall, as JPEG.
fn preview_jpeg(frame: &crate::camera::CamFrame, ws: &WebcamSettings) -> Result<Vec<u8>, String> {
    let crop = camera_crop(frame.width, frame.height, ws.shape);
    let (w, h) = ((crop.w * PREVIEW_HEIGHT / crop.h.max(1)).max(1), PREVIEW_HEIGHT);
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        let sy = crop.y as u32 + y * crop.h / h;
        for x in 0..w {
            let px = if ws.mirror { w - 1 - x } else { x };
            let sx = crop.x as u32 + px * crop.w / w;
            let i = ((sy * frame.width + sx) * 4) as usize;
            let p = frame.bgra.get(i..i + 3).ok_or("Khung camera không hợp lệ")?;
            rgb.extend_from_slice(&[p[2], p[1], p[0]]);
        }
    }
    let img = image::RgbImage::from_raw(w, h, rgb).ok_or("Khung camera không hợp lệ")?;
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 70).encode_image(&img).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

pub const CAM_WINDOW: &str = "cam";
pub const CAM_STATE: &str = "cam-state";
pub const CAM_FRAME: &str = "cam-frame";

#[derive(Clone, Serialize)]
struct CamStatePayload {
    on: bool,
}

#[derive(Clone, Serialize)]
struct CamFramePayload {
    jpeg: String,
}

struct CamSession {
    // Dropped last: stops reading and releases the device.
    _camera: Camera,
    stop_preview: Arc<AtomicBool>,
    preview: Option<std::thread::JoinHandle<()>>,
}

impl Drop for CamSession {
    fn drop(&mut self) {
        self.stop_preview.store(true, Ordering::SeqCst);
        if let Some(t) = self.preview.take() {
            let _ = t.join();
        }
    }
}

/// The webcam of the current recording, if on; and whether it was already started this session.
#[derive(Default)]
pub struct CamState {
    session: Mutex<Option<CamSession>>,
    attempted: AtomicBool,
    /// Set while a camera is being opened, so a second start does not open it twice.
    starting: AtomicBool,
    gate: StartGate,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Turns the webcam off: out of the video, bubble closed, device released.
pub fn stop_camera(app: &AppHandle) {
    let (Some(state), Some(cams)) = (app.try_state::<AppState>(), app.try_state::<CamState>()) else { return };
    cams.gate.cancel();
    state.recorder.live().set_camera(None);
    let session = lock(&cams.session).take();
    drop(session);
    if let Some(w) = app.get_webview_window(CAM_WINDOW) {
        let _ = w.close();
    }
    let _ = app.emit(CAM_STATE, CamStatePayload { on: false });
}

/// Turns the webcam on (into the video, with an on-screen bubble) or off.
pub async fn set_camera(app: AppHandle, on: bool) -> Result<(), String> {
    if !on {
        stop_camera(&app);
        return Ok(());
    }
    let state = app.state::<AppState>();
    let cams = app.state::<CamState>();
    let live = state.recorder.live();
    if !recording(&state) {
        return Err("Chỉ bật webcam được khi đang quay".to_string());
    }
    if lock(&cams.session).is_some() || cams.starting.swap(true, Ordering::SeqCst) {
        return Ok(()); // already on, or being opened
    }
    let ticket = cams.gate.begin();
    let settings = state.settings();
    let ws = settings.webcam.clone();
    let device = ws.device.clone();
    let opened = tauri::async_runtime::spawn_blocking(move || Camera::open(device.as_deref())).await.map_err(|e| e.to_string());
    cams.starting.store(false, Ordering::SeqCst);
    let camera = opened??;
    // The recording may have stopped while the camera was opening: then let it go at once.
    if !cams.gate.still_valid(ticket) || !recording(&state) {
        drop(camera);
        return Ok(());
    }
    let target = state.recorder.last_target().ok_or("Chưa chọn vùng quay")?;
    let area = overlay_rect(capture::visible_rect(&target)?);
    let bubble = bubble_rect(area.w, area.h, &ws);
    live.set_camera(Some((camera.latest(), ws.clone())));

    let on_screen = bubble_on_screen(area, bubble);
    let window = match app.get_webview_window(CAM_WINDOW) {
        Some(w) => w,
        None => WebviewWindowBuilder::new(&app, CAM_WINDOW, WebviewUrl::App("index.html".into()))
            .title("HP Screen - Webcam")
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .focused(false)
            .visible(false)
            .build()
            .map_err(|e| format!("Không mở được khung webcam: {e}"))?,
    };
    let _ = window.set_position(PhysicalPosition::new(on_screen.x, on_screen.y));
    let _ = window.set_size(PhysicalSize::new(on_screen.w, on_screen.h));
    exclude_from_capture(&window);
    let _ = window.set_ignore_cursor_events(true);
    let _ = window.show();

    // Send the bubble small pictures; notice when the camera stops.
    let stop_preview = Arc::new(AtomicBool::new(false));
    let preview = {
        let (app, stop, latest, alive_cam) = (app.clone(), stop_preview.clone(), camera.latest(), CameraAlive(camera.alive_flag()));
        std::thread::Builder::new()
            .name("hp-cam-preview".into())
            .spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    if !alive_cam.get() {
                        crate::events::rec_error(&app, "Camera đã ngắt kết nối nên webcam bị tắt; việc quay vẫn tiếp tục.");
                        // Stopping joins this thread: hand it to another one.
                        let app = app.clone();
                        std::thread::spawn(move || stop_camera(&app));
                        return;
                    }
                    let frame = lock(&latest).clone();
                    if let Some(jpeg) = frame.and_then(|f| preview_jpeg(&f, &ws).ok()) {
                        use base64::Engine;
                        let jpeg = base64::engine::general_purpose::STANDARD.encode(jpeg);
                        let _ = app.emit_to(CAM_WINDOW, CAM_FRAME, CamFramePayload { jpeg });
                    }
                    std::thread::sleep(PREVIEW_EVERY);
                }
            })
            .map_err(|e| e.to_string())?
    };
    *lock(&cams.session) = Some(CamSession { _camera: camera, stop_preview, preview: Some(preview) });
    let _ = app.emit(CAM_STATE, CamStatePayload { on: true });
    Ok(())
}

struct CameraAlive(Arc<AtomicBool>);

impl CameraAlive {
    fn get(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Called on every recording state report: starts the webcam when a recording begins with
/// the webcam enabled, and forgets the attempt when the session ends.
pub fn on_rec_state(app: &AppHandle, state: RecState) {
    sync_region_border(app, state);
    let Some(cams) = app.try_state::<CamState>() else { return };
    if state == RecState::Idle {
        cams.attempted.store(false, Ordering::SeqCst);
        return;
    }
    let enabled = app.try_state::<AppState>().is_some_and(|s| s.settings().webcam.enabled);
    // Several reports say Recording; only the first may start the webcam.
    if should_auto_start(state, enabled, cams.attempted.load(Ordering::SeqCst)) && !cams.attempted.swap(true, Ordering::SeqCst) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(message) = set_camera(app.clone(), true).await {
                crate::events::rec_error(&app, &message);
            }
        });
    }
}

#[tauri::command]
pub fn cam_list() -> Vec<CameraInfo> {
    crate::camera::list_cameras()
}

#[tauri::command]
pub async fn cam_set(app: AppHandle, on: bool) -> Result<(), String> {
    set_camera(app, on).await
}

#[tauri::command]
pub fn cam_on(cams: State<'_, CamState>) -> bool {
    lock(&cams.session).is_some()
}

/// Thickness of the on-screen border around a recorded region, in physical pixels. The
/// border lies entirely outside the region, so it never covers what is recorded.
pub const BORDER_PX: i32 = 6;
pub const BORDER_WINDOW: &str = "region-border";

/// The border window: the region grown by `BORDER_PX` on every side.
fn border_rect(region: Rect) -> Rect {
    let grow = (2 * BORDER_PX) as u32;
    Rect { x: region.x - BORDER_PX, y: region.y - BORDER_PX, w: region.w + grow, h: region.h + grow }
}

/// Whether the region border is on screen: from the countdown until the recording stops.
fn shows_border(state: RecState, target: Option<&Target>) -> bool {
    matches!(state, RecState::Countdown | RecState::Recording | RecState::Paused) && matches!(target, Some(Target::Region { .. }))
}

/// Set while the border window is being made, so state reports arriving meanwhile do not make
/// a second one.
static BORDER_OPENING: AtomicBool = AtomicBool::new(false);

/// Shows or hides the dashed border around a recorded region. It is excluded from capture and
/// lets the mouse through; being outside the region, it could not reach the video anyway.
fn sync_region_border(app: &AppHandle, state: RecState) {
    let target = app.try_state::<AppState>().and_then(|s| s.recorder.last_target());
    let window = app.get_webview_window(BORDER_WINDOW);
    if !shows_border(state, target.as_ref()) {
        if let Some(w) = window {
            let _ = w.close();
        }
        return;
    }
    if window.is_some() || BORDER_OPENING.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some(target) = target else { return };
    let app = app.clone();
    // Windows must not be made on the event-loop thread.
    tauri::async_runtime::spawn(async move {
        let made = (|| -> Result<(), String> {
            let area = border_rect(capture::target_rect(&target)?);
            let w = WebviewWindowBuilder::new(&app, BORDER_WINDOW, WebviewUrl::App("index.html".into()))
                .title("HP Screen - Vùng quay")
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .focused(false)
                .visible(false)
                .build()
                .map_err(|e| e.to_string())?;
            let _ = w.set_position(PhysicalPosition::new(area.x, area.y));
            let _ = w.set_size(PhysicalSize::new(area.w, area.h));
            exclude_from_capture(&w);
            let _ = w.set_ignore_cursor_events(true);
            // The session may have ended while the window was being made.
            let still = app.try_state::<AppState>().is_some_and(|s| shows_border(s.recorder.status().state, s.recorder.last_target().as_ref()));
            if still {
                let _ = w.show();
            } else {
                let _ = w.close();
            }
            Ok(())
        })();
        BORDER_OPENING.store(false, Ordering::SeqCst);
        if let Err(e) = made {
            crate::paths::log_error(&app, &format!("Không hiện được viền vùng quay: {e}"));
        }
    });
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
            let area = overlay_rect(capture::visible_rect(&target)?);
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
            // The recording may have ended while the window was being made.
            if !recording(&state) {
                let _ = w.close();
                return Ok(());
            }
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

/// Closes the live windows when a recording ends, and turns the webcam off.
pub fn close_windows(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(DRAW_WINDOW) {
        let _ = w.close();
    }
    let _ = app.emit(DRAW_MODE, DrawMode { on: false });
    // Releasing the camera joins its threads: keep it off the caller's (event loop) thread.
    let app = app.clone();
    std::thread::spawn(move || stop_camera(&app));
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
    fn the_border_sits_just_outside_the_region() {
        assert_eq!(border_rect(Rect { x: 100, y: 50, w: 640, h: 480 }), Rect { x: 94, y: 44, w: 652, h: 492 });
        assert_eq!(border_rect(Rect { x: -1920, y: 0, w: 300, h: 200 }), Rect { x: -1926, y: -6, w: 312, h: 212 });
    }

    #[test]
    fn the_border_shows_only_for_a_region_while_a_session_runs() {
        let region = Target::Region { monitor: 0, rect: Rect { x: 0, y: 0, w: 100, h: 100 } };
        let monitor = Target::Monitor { index: 0 };
        for state in [RecState::Countdown, RecState::Recording, RecState::Paused] {
            assert!(shows_border(state, Some(&region)), "{state:?}");
        }
        assert!(!shows_border(RecState::Idle, Some(&region)));
        assert!(!shows_border(RecState::Finalizing, Some(&region)));
        assert!(!shows_border(RecState::Recording, Some(&monitor)));
        assert!(!shows_border(RecState::Recording, None));
    }

    #[test]
    fn a_start_overtaken_by_a_stop_is_abandoned() {
        let gate = StartGate::default();
        let ticket = gate.begin();
        assert!(gate.still_valid(ticket));
        gate.cancel();
        assert!(!gate.still_valid(ticket), "a stop during the slow camera open voids the start");
        let next = gate.begin();
        assert!(gate.still_valid(next));
        let overtaken = next;
        let newer = gate.begin();
        assert!(!gate.still_valid(overtaken) && gate.still_valid(newer), "only the latest start counts");
    }

    #[test]
    fn bubble_window_rect_on_screen() {
        let area = Rect { x: -1920, y: 100, w: 1920, h: 1080 };
        let bubble = Rect { x: 1677, y: 837, w: 216, h: 216 };
        assert_eq!(bubble_on_screen(area, bubble), Rect { x: -243, y: 937, w: 216, h: 216 });
    }

    #[test]
    fn webcam_starts_once_per_recording_when_enabled() {
        assert!(should_auto_start(RecState::Recording, true, false));
        assert!(!should_auto_start(RecState::Recording, true, true), "already tried this session");
        assert!(!should_auto_start(RecState::Recording, false, false), "turned off in settings");
        assert!(!should_auto_start(RecState::Countdown, true, false), "not before recording starts");
    }

    #[test]
    fn preview_picture_is_small_and_square_for_a_circle() {
        let frame = crate::camera::CamFrame { width: 640, height: 480, bgra: vec![200; 640 * 480 * 4], at_ms: 0 };
        let jpeg = preview_jpeg(&frame, &hp_core::webcam::WebcamSettings::default()).unwrap();
        let img = image::load_from_memory(&jpeg).unwrap();
        assert_eq!((img.width(), img.height()), (240, 240));
    }

    #[test]
    fn draw_window_rect_matches_the_target() {
        assert_eq!(overlay_rect(Rect { x: -1920, y: 10, w: 1921, h: 1081 }), Rect { x: -1920, y: 10, w: 1920, h: 1080 });
        assert_eq!(overlay_rect(Rect { x: 0, y: 0, w: 2560, h: 1600 }), Rect { x: 0, y: 0, w: 2560, h: 1600 });
    }
}
