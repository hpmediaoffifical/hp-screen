//! Tauri commands: the only way the UI reaches the core.
use crate::capture::{self, MonitorInfo, Target, WindowInfo};
use crate::events;
use crate::recorder::{RecStatus, Recorder};
use crate::screenshot;
use hp_core::settings::Settings;
use std::sync::Mutex;
use tauri::{AppHandle, State};

/// Shared application state.
pub struct AppState {
    pub settings: Mutex<Settings>,
    pub recorder: Recorder,
}

impl AppState {
    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[tauri::command]
pub fn list_monitors() -> Vec<MonitorInfo> {
    capture::list_monitors()
}

#[tauri::command]
pub fn list_windows() -> Vec<WindowInfo> {
    capture::list_windows()
}

/// Saves a PNG of `target`, copies it to the clipboard, and returns the file path.
#[tauri::command]
pub async fn take_screenshot(app: AppHandle, state: State<'_, AppState>, target: Target) -> Result<String, String> {
    let settings = state.settings();
    // HP Screen's own windows are excluded from capture, so they never appear in it.
    let path = screenshot::take_screenshot(&app, &settings, &target)?;
    events::library_changed(&app);
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn rec_start(state: State<'_, AppState>, target: Target) -> Result<(), String> {
    state.recorder.start(target, state.settings())
}

#[tauri::command]
pub fn rec_stop(state: State<'_, AppState>) -> Result<(), String> {
    state.recorder.stop()
}

#[tauri::command]
pub fn rec_toggle_pause(state: State<'_, AppState>) -> Result<(), String> {
    state.recorder.toggle_pause()
}

#[tauri::command]
pub fn rec_cancel(state: State<'_, AppState>) -> Result<(), String> {
    state.recorder.cancel()
}

#[tauri::command]
pub fn rec_status(state: State<'_, AppState>) -> RecStatus {
    state.recorder.status()
}

/// Whether a default playback device and a default microphone exist right now.
#[tauri::command]
pub fn audio_status() -> crate::audio::AudioStatus {
    crate::audio::audio_status()
}

/// How the last hotkey registration went, one entry per action.
#[tauri::command]
pub fn hotkey_status(registry: State<'_, crate::hotkeys::HotkeyRegistry>) -> Vec<crate::hotkeys::HotkeyStatus> {
    registry.inner().0.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Opens the selection overlay; resolves to the chosen target or `None` when cancelled.
#[tauri::command]
pub async fn pick_target(app: AppHandle, mode: crate::picker::PickMode) -> Result<Option<Target>, String> {
    crate::picker::pick(app, mode).await
}

#[tauri::command]
pub fn picker_context(app: AppHandle) -> Option<crate::picker::PickerContext> {
    crate::picker::context(&app)
}

#[tauri::command]
pub fn picker_done(app: AppHandle, target: Option<Target>) {
    crate::picker::finish(&app, target);
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> Settings {
    state.settings()
}

/// Validates, saves and applies new settings; returns what was actually stored.
#[tauri::command]
pub fn settings_set(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> Result<Settings, String> {
    let settings = hp_core::settings::normalize(settings);
    crate::paths::write_settings(&app, &settings)?;
    let previous = std::mem::replace(&mut *state.settings.lock().unwrap_or_else(|e| e.into_inner()), settings.clone());
    // Re-registering briefly drops the hotkeys, so only do it when one of them changed.
    let hotkeys = |s: &Settings| {
        [s.hotkey_toggle.clone(), s.hotkey_pause.clone(), s.hotkey_screenshot.clone(), s.hotkey_quickshot.clone()]
    };
    if hotkeys(&previous) != hotkeys(&settings) {
        crate::hotkeys::register_hotkeys(&app, &settings);
    }
    {
        use tauri_plugin_autostart::ManagerExt;
        let autostart = app.autolaunch();
        // Best effort: a failure here must not lose the other settings.
        let _ = if settings.autostart { autostart.enable() } else { autostart.disable() };
    }
    // Every window keeps its own copy of the settings; tell them all.
    let _ = tauri::Emitter::emit(&app, "settings-changed", &settings);
    Ok(settings)
}

fn library_dir(app: &AppHandle, state: &State<'_, AppState>) -> Result<std::path::PathBuf, String> {
    crate::paths::output_dir(app, &state.settings())
}

/// A library path must be a media file directly inside the output folder: the UI only ever
/// sends such paths, and nothing else may be renamed, deleted or opened through these commands.
fn library_item(app: &AppHandle, state: &State<'_, AppState>, path: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::PathBuf::from(path);
    let dir = library_dir(app, state)?;
    let inside = path.parent().is_some_and(|p| p == dir.as_path());
    if !inside || hp_core::library::kind_of(&path).is_none() || !path.is_file() {
        return Err("File không thuộc thư viện HP Screen".to_string());
    }
    Ok(path)
}

#[tauri::command]
pub fn library_list(
    app: AppHandle,
    state: State<'_, AppState>,
    filter: Option<hp_core::library::Kind>,
) -> Result<Vec<crate::library_fs::LibraryEntry>, String> {
    Ok(crate::library_fs::list(&library_dir(&app, &state)?, filter))
}

#[tauri::command]
pub fn library_rename(app: AppHandle, state: State<'_, AppState>, path: String, new_stem: String) -> Result<String, String> {
    let renamed = crate::library_fs::rename(&library_item(&app, &state, &path)?, &new_stem)?;
    events::library_changed(&app);
    Ok(renamed.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn library_delete(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<(), String> {
    crate::library_fs::delete(&library_item(&app, &state, &path)?)?;
    events::library_changed(&app);
    Ok(())
}

/// Opens the file with the default Windows app.
#[tauri::command]
pub fn library_open(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let path = library_item(&app, &state, &path)?;
    app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| format!("Không mở được file: {e}"))
}

/// Shows the file in File Explorer, or the output folder when no file is given.
#[tauri::command]
pub fn library_reveal(app: AppHandle, state: State<'_, AppState>, path: Option<String>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    match path {
        Some(path) => app
            .opener()
            .reveal_item_in_dir(library_item(&app, &state, &path)?)
            .map_err(|e| format!("Không mở được thư mục: {e}")),
        None => app
            .opener()
            .open_path(library_dir(&app, &state)?.to_string_lossy(), None::<&str>)
            .map_err(|e| format!("Không mở được thư mục: {e}")),
    }
}

/// The file name the pattern would give right now, for the live preview in Settings.
#[tauri::command]
pub fn preview_name(pattern: String) -> String {
    format!("{}.mp4", hp_core::naming::render_pattern(&pattern, crate::paths::now_local()))
}

/// Folder chooser for the output folder; `None` when the user cancels.
#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app.dialog().file().set_title("Chọn thư mục lưu video và ảnh").blocking_pick_folder()?;
    picked.into_path().ok().map(|p| p.to_string_lossy().into_owned())
}

/// The folder recordings currently go to (the default when none is set).
#[tauri::command]
pub fn output_folder(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    library_dir(&app, &state).map(|p| p.to_string_lossy().into_owned())
}

/// Tells the core what is selected on the record page, so hotkeys and the tray act on it.
#[tauri::command]
pub fn set_target(app: AppHandle, state: State<'_, AppState>, target: Target) {
    let _ = tauri::Emitter::emit(&app, "target-changed", &target);
    state.recorder.set_target(target);
}

/// Opens "capture and annotate" from the main window.
#[tauri::command]
pub async fn snip_start(app: AppHandle) -> Result<(), String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || crate::snip::start(&handle))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn snip_context(app: AppHandle) -> Option<crate::snip::SnipContext> {
    crate::snip::context(&app)
}

/// The overlay is drawn and ready: show it.
#[tauri::command]
pub fn snip_ready(app: AppHandle) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window(crate::snip::SNIP) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Copies and/or saves the annotated picture; returns the saved file, if any.
#[tauri::command]
pub async fn snip_finish(app: AppHandle, action: crate::snip::SnipAction, png: String) -> Result<Option<String>, String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || crate::snip::finish(&handle, action, &png))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn snip_cancel(app: AppHandle) {
    crate::snip::close(&app);
}

/// Switches to the compact bar.
#[tauri::command]
pub fn compact_show(app: AppHandle) {
    crate::shell::show_compact(&app);
}

/// Hides the compact bar; the app stays in the tray.
#[tauri::command]
pub fn compact_hide(app: AppHandle) {
    crate::shell::hide_compact(&app);
}

/// Opens the full HP Screen window.
#[tauri::command]
pub fn open_main(app: AppHandle) {
    crate::shell::show_main(&app);
}

/// What hotkeys, the tray and the compact bar record right now.
#[tauri::command]
pub fn get_target(state: State<'_, AppState>) -> Target {
    state.recorder.last_target().unwrap_or_else(crate::hotkeys::default_target)
}

/// The frozen screen as raw image bytes. Sent through IPC rather than loaded from a file URL:
/// a picture from another origin would lock the drawing canvas and block exporting it.
#[tauri::command]
pub fn snip_frozen(app: AppHandle) -> Result<tauri::ipc::Response, String> {
    let ctx = crate::snip::context(&app).ok_or("Công cụ chụp ảnh chưa được mở")?;
    let bytes = std::fs::read(&ctx.frozen).map_err(|e| format!("Không đọc được ảnh màn hình: {e}"))?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Pins the annotated picture on screen where its area was, then closes the overlay.
#[tauri::command]
pub async fn snip_pin(app: AppHandle, png: String, rect: hp_core::geometry::Rect) -> Result<(), String> {
    crate::pin::open(&app, &png, rect)
}

/// Reads the text in a picture (Windows OCR, offline).
#[tauri::command]
pub async fn snip_ocr(png: String) -> Result<crate::ocr::OcrOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || crate::ocr::recognize(&crate::snip::decode_png(&png)?))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn copy_text(text: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::ocr::copy_text(&text)).await.map_err(|e| e.to_string())?
}

/// The calling pin window's picture (PNG bytes).
#[tauri::command]
pub fn pin_image(window: tauri::WebviewWindow) -> Result<tauri::ipc::Response, String> {
    Ok(tauri::ipc::Response::new(crate::pin::image(&window)?))
}

#[tauri::command]
pub async fn pin_copy(window: tauri::WebviewWindow) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::pin::copy(&window)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn pin_close(window: tauri::WebviewWindow) {
    crate::pin::close(&window);
}

#[tauri::command]
pub fn pin_ready(window: tauri::WebviewWindow) {
    crate::pin::ready(&window);
}

/// Starts a scrolling capture of `rect` (the overlay's selection, physical pixels).
#[tauri::command]
pub async fn snip_scroll_start(app: AppHandle, rect: hp_core::geometry::Rect) -> Result<(), String> {
    crate::scroll::start(&app, rect)
}

/// Ends the scrolling capture; with `keep`, saves and copies it and returns (path, width, height).
#[tauri::command]
pub async fn snip_scroll_stop(app: AppHandle, keep: bool) -> Result<Option<(String, u32, u32)>, String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || crate::scroll::stop(&handle, keep)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn scroll_close(app: AppHandle) {
    crate::scroll::close_bar(&app);
}
