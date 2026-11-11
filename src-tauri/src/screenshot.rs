//! Screenshots: one frame at physical resolution, saved as PNG and copied to the clipboard.
use crate::capture::{Target, grab_frame};
use crate::paths::{new_media_path, output_dir};
use hp_core::library::thumb_path;
use hp_core::settings::Settings;
use image::RgbaImage;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

const THUMB_WIDTH: u32 = 320;

/// Captures `target` now, saves it and copies it to the clipboard.
pub fn take_screenshot(app: &AppHandle, settings: &Settings, target: &Target) -> Result<PathBuf, String> {
    let dir = output_dir(app, settings)?;
    let image = grab_frame(target)?;
    let path = save_png(&dir, settings, &image)?;
    copy_to_clipboard(&image);
    Ok(path)
}

/// Saves `image` in `dir` under a new name from the user's pattern, with a library preview.
pub fn save_png(dir: &Path, settings: &Settings, image: &RgbaImage) -> Result<PathBuf, String> {
    let path = new_media_path(dir, settings, "png");
    image.save(&path).map_err(|e| format!("Không lưu được ảnh chụp: {e}"))?;
    // Preview for the library; best effort, and never larger than the picture.
    let thumb_width = THUMB_WIDTH.min(image.width()).max(1);
    let thumb_height = (image.height() as u64 * thumb_width as u64 / image.width().max(1) as u64).max(1) as u32;
    let _ = image::imageops::thumbnail(image, thumb_width, thumb_height).save(thumb_path(&path));
    Ok(path)
}

/// Best effort: a busy clipboard must not turn a finished screenshot into an error.
pub fn copy_to_clipboard(image: &RgbaImage) {
    let (width, height) = (image.width() as usize, image.height() as usize);
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_image(arboard::ImageData { width, height, bytes: image.as_raw().into() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("hp-screen-tests").join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(hp_core::library::DATA_DIR)).unwrap();
        dir
    }

    #[test]
    fn saving_writes_the_picture_and_a_small_preview() {
        let dir = fresh_dir("save-png");
        let settings = Settings { name_pattern: "anh".to_string(), ..Settings::default() };
        let image = RgbaImage::from_pixel(1280, 400, image::Rgba([10, 200, 30, 255]));
        let path = save_png(&dir, &settings, &image).unwrap();
        assert_eq!(path, dir.join("anh.png"));
        assert_eq!(image::image_dimensions(&path).unwrap(), (1280, 400));
        assert_eq!(image::image_dimensions(thumb_path(&path)).unwrap(), (320, 100));
    }

    #[test]
    fn a_second_save_gets_its_own_name() {
        let dir = fresh_dir("save-png-twice");
        let settings = Settings { name_pattern: "anh".to_string(), ..Settings::default() };
        let image = RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 255]));
        assert_eq!(save_png(&dir, &settings, &image).unwrap(), dir.join("anh.png"));
        assert_eq!(save_png(&dir, &settings, &image).unwrap(), dir.join("anh_2.png"));
    }

    #[test]
    fn pictures_smaller_than_the_preview_are_not_enlarged() {
        let dir = fresh_dir("save-png-small");
        let settings = Settings { name_pattern: "nho".to_string(), ..Settings::default() };
        let path = save_png(&dir, &settings, &RgbaImage::from_pixel(40, 3, image::Rgba([1, 2, 3, 255]))).unwrap();
        assert_eq!(image::image_dimensions(thumb_path(&path)).unwrap(), (40, 3));
    }
}
