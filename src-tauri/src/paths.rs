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

/// Below this much free space a recording is stopped (or not started) so the file can be closed.
pub const MIN_FREE_BYTES: u64 = 500 * 1024 * 1024;

/// Free bytes on the drive holding `dir`; `None` when Windows cannot tell.
pub fn free_space(dir: &Path) -> Option<u64> {
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    use windows::core::HSTRING;
    let mut free = 0u64;
    // SAFETY: the path is a valid wide string and the out pointer is valid.
    unsafe { GetDiskFreeSpaceExW(&HSTRING::from(dir.as_os_str()), Some(&mut free), None, None) }.ok()?;
    Some(free)
}

/// Checks that a recording can be saved in `dir`: it must be writable and have room.
pub fn ensure_recordable(dir: &Path) -> Result<(), String> {
    let probe = dir.join(DATA_DIR).join(".write-test");
    std::fs::write(&probe, b"")
        .map_err(|e| format!("Không ghi được vào thư mục lưu \"{}\": {e}. Hãy chọn thư mục khác trong Cài đặt.", dir.display()))?;
    let _ = std::fs::remove_file(&probe);
    if free_space(dir).is_some_and(|free| free < MIN_FREE_BYTES) {
        return Err("Ổ đĩa chứa thư mục lưu còn dưới 500 MB trống. Hãy giải phóng dung lượng hoặc chọn thư mục khác.".to_string());
    }
    Ok(())
}

/// Appends a line to the app's log file; used for errors the user may want to send us.
pub fn log_error(app: &AppHandle, message: &str) {
    use std::io::Write;
    let Ok(dir) = app.path().app_log_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("hp-screen.log")) {
        let t = now_local();
        let _ = writeln!(file, "{:04}-{:02}-{:02} {:02}:{:02}:{:02}  {message}", t.year, t.month, t.day, t.hour, t.minute, t.second);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_space_is_known_for_the_temp_folder() {
        assert!(free_space(&std::env::temp_dir()).is_some_and(|free| free > 0));
        assert_eq!(free_space(Path::new(r"Q:\khong\ton\tai")), None);
    }

    #[test]
    fn a_missing_folder_is_not_recordable() {
        let error = ensure_recordable(Path::new(r"Q:\khong\ton\tai")).unwrap_err();
        assert!(error.contains("Cài đặt"), "the message tells the user where to fix it: {error}");
    }

    #[test]
    fn a_normal_folder_is_recordable_and_left_clean() {
        let dir = std::env::temp_dir().join("hp-screen-tests").join(format!("recordable-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(DATA_DIR)).unwrap();
        ensure_recordable(&dir).unwrap();
        assert_eq!(std::fs::read_dir(dir.join(DATA_DIR)).unwrap().count(), 0, "the write probe is removed");
    }
}
