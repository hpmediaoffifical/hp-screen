//! Screenshots: one frame at physical resolution, saved as PNG and copied to the clipboard.
use crate::capture::{Target, grab_frame};
use crate::paths::{new_media_path, output_dir};
use hp_core::settings::Settings;
use std::path::PathBuf;
use tauri::AppHandle;

pub fn take_screenshot(app: &AppHandle, settings: &Settings, target: &Target) -> Result<PathBuf, String> {
    let dir = output_dir(app, settings)?;
    let image = grab_frame(target)?;
    let path = new_media_path(&dir, settings, "png");
    image.save(&path).map_err(|e| format!("Không lưu được ảnh chụp: {e}"))?;

    // The file is already saved; a busy clipboard must not turn the screenshot into an error.
    let (width, height) = (image.width() as usize, image.height() as usize);
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_image(arboard::ImageData { width, height, bytes: image.into_raw().into() });
    }
    Ok(path)
}
