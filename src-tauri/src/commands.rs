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
