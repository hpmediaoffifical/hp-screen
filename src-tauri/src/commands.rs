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
    *state.settings.lock().unwrap_or_else(|e| e.into_inner()) = settings.clone();
    crate::hotkeys::register_hotkeys(&app, &settings);
    Ok(settings)
}
