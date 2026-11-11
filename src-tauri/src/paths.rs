//! Where files go: the output folder, its data sub-folder, new media file names, settings file.
use hp_core::library::DATA_DIR;
use hp_core::naming::{LocalTime, render_pattern, unique_path};
use hp_core::settings::{Settings, load_settings};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use windows::Win32::System::SystemInformation::GetLocalTime;

pub const DEFAULT_FOLDER_NAME: &str = "HP Screen";

/// The folder recordings are saved to, created together with its data sub-folder.
pub fn output_dir(app: &AppHandle, settings: &Settings) -> Result<PathBuf, String> {
    let dir = match &settings.output_dir {
        Some(dir) => dir.clone(),
        None => app.path().video_dir().map_err(|e| e.to_string())?.join(DEFAULT_FOLDER_NAME),
    };
    std::fs::create_dir_all(dir.join(DATA_DIR))
        .map_err(|e| format!("Không tạo được thư mục lưu \"{}\": {e}", dir.display()))?;
    Ok(dir)
}

pub fn now_local() -> LocalTime {
    // SAFETY: GetLocalTime has no preconditions.
    let t = unsafe { GetLocalTime() };
    LocalTime {
        year: t.wYear,
        month: t.wMonth as u8,
        day: t.wDay as u8,
        hour: t.wHour as u8,
        minute: t.wMinute as u8,
        second: t.wSecond as u8,
    }
}

/// A file path in `dir` that does not exist yet, named from the user's pattern and the time now.
pub fn new_media_path(dir: &Path, settings: &Settings, ext: &str) -> PathBuf {
    let stem = render_pattern(&settings.name_pattern, now_local());
    unique_path(dir, &stem, ext, &|p| p.exists())
}

fn settings_file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

/// Reads settings; a missing or damaged file gives the defaults.
pub fn read_settings(app: &AppHandle) -> Settings {
    settings_file(app)
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|json| load_settings(&json))
        .unwrap_or_default()
}

pub fn write_settings(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(settings_file(app)?, json).map_err(|e| format!("Không lưu được cài đặt: {e}"))
}
