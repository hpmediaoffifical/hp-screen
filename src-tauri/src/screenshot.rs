//! Screenshots: one frame at physical resolution, saved as PNG and copied to the clipboard.
use crate::capture::{Target, grab_frame};
use crate::paths::{new_media_path, output_dir};
use hp_core::library::thumb_path;
use hp_core::settings::Settings;
use std::path::PathBuf;
use tauri::AppHandle;

const THUMB_WIDTH: u32 = 320;

pub fn take_screenshot(app: &AppHandle, settings: &Settings, target: &Target) -> Result<PathBuf, String> {
    let dir = output_dir(app, settings)?;
    let image = grab_frame(target)?;
    let path = new_media_path(&dir, settings, "png");
    image.save(&path).map_err(|e| format!("Không lưu được ảnh chụp: {e}"))?;

    // Preview for the library; best effort.
    let thumb_height = (image.height() as u64 * THUMB_WIDTH as u64 / image.width().max(1) as u64).max(1) as u32;
    let _ = image::imageops::thumbnail(&image, THUMB_WIDTH.min(image.width()), thumb_height.min(image.height()))
        .save(thumb_path(&path));

    // The file is already saved; a busy clipboard must not turn the screenshot into an error.
    let (width, height) = (image.width() as usize, image.height() as usize);
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_image(arboard::ImageData { width, height, bytes: image.into_raw().into() });
    }
    Ok(path)
}
