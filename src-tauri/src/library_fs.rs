//! The library on disk: listing the output folder and renaming or deleting an item together
//! with the files that accompany it.
use hp_core::library::{Item, Kind, MediaMeta, companion_paths, kind_of, meta_path, sort_and_filter, thumb_path};
use hp_core::naming::sanitize;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// One row of the library as the UI shows it.
#[derive(Clone, Debug, Serialize)]
pub struct LibraryEntry {
    pub path: String,
    pub name: String,
    pub kind: Kind,
    pub created_ms: i64,
    pub size: u64,
    /// Preview image file, when one exists.
    pub thumb: Option<String>,
    pub duration_ms: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Videos and screenshots directly inside `dir`, newest first.
pub fn list(dir: &Path, filter: Option<Kind>) -> Vec<LibraryEntry> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let items: Vec<Item> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let kind = kind_of(&path)?;
            let meta = entry.metadata().ok().filter(|m| m.is_file())?;
            let created = meta.created().or_else(|_| meta.modified()).ok()?;
            let created_ms = created.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
            Some(Item { path, kind, created_ms, size: meta.len() })
        })
        .collect();
    sort_and_filter(items, filter).into_iter().map(describe).collect()
}

fn describe(item: Item) -> LibraryEntry {
    let thumb = Some(thumb_path(&item.path)).filter(|p| p.exists());
    let (duration_ms, width, height) = match item.kind {
        Kind::Video => match read_meta(&item.path) {
            Some(m) => (Some(m.duration_ms), Some(m.width), Some(m.height)),
            None => (None, None, None),
        },
        // Reads only the PNG header.
        Kind::Image => match image::image_dimensions(&item.path) {
            Ok((w, h)) => (None, Some(w), Some(h)),
            Err(_) => (None, None, None),
        },
    };
    LibraryEntry {
        name: item.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        path: item.path.to_string_lossy().into_owned(),
        kind: item.kind,
        created_ms: item.created_ms,
        size: item.size,
        thumb: thumb.map(|p| p.to_string_lossy().into_owned()),
        duration_ms,
        width,
        height,
    }
}

fn read_meta(media: &Path) -> Option<MediaMeta> {
    serde_json::from_str(&std::fs::read_to_string(meta_path(media)).ok()?).ok()
}

/// Records what is known about a finished recording so the library need not open the video.
pub fn write_meta(media: &Path, meta: &MediaMeta) {
    if let Ok(json) = serde_json::to_string(meta) {
        let _ = std::fs::write(meta_path(media), json);
    }
}

/// Whether any video or screenshot in `dir` is already called `stem` (ignoring case).
pub fn stem_in_use(dir: &Path, stem: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    entries.flatten().map(|e| e.path()).any(|p| {
        kind_of(&p).is_some() && p.file_stem().is_some_and(|s| s.to_string_lossy().to_lowercase() == stem.to_lowercase())
    })
}

/// Renames `path` to `new_stem` (extension kept) along with its companion files.
pub fn rename(path: &Path, new_stem: &str) -> Result<PathBuf, String> {
    if new_stem.trim().is_empty() {
        return Err("Tên file không được để trống".to_string());
    }
    let stem = sanitize(new_stem);
    let ext = path.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    let target = path.with_file_name(format!("{stem}.{ext}"));
    if target == path {
        return Ok(target);
    }
    let same_item = path.file_stem().is_some_and(|s| s.to_string_lossy().to_lowercase() == stem.to_lowercase());
    let taken = !same_item && path.parent().is_some_and(|dir| stem_in_use(dir, &stem));
    if taken || (!same_item && target.exists()) {
        return Err(format!("Đã có video hoặc ảnh tên \"{stem}\" trong thư mục"));
    }
    std::fs::rename(path, &target).map_err(|e| format!("Không đổi tên được: {e}"))?;
    for (from, to) in companion_paths(path).iter().zip(companion_paths(&target)) {
        if from.exists() {
            let _ = std::fs::rename(from, to);
        }
    }
    Ok(target)
}

/// Sends `path` to the Recycle Bin and removes its companion files.
pub fn delete(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|e| format!("Không xóa được file: {e}"))?;
    for companion in companion_paths(path) {
        let _ = std::fs::remove_file(companion);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder with one "video", one "image", their companions and an unrelated file.
    fn fixture(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("hp-screen-tests").join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".hpscreen")).unwrap();
        std::fs::write(dir.join("hp_a.mp4"), b"video").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        image::RgbaImage::new(3, 2).save(dir.join("hp_b.png")).unwrap();
        for companion in companion_paths(&dir.join("hp_a.mp4")) {
            std::fs::write(companion, b"{}").unwrap();
        }
        write_meta(&dir.join("hp_a.mp4"), &MediaMeta { duration_ms: 4200, width: 640, height: 480 });
        dir
    }

    #[test]
    fn list_shows_media_only_with_their_details() {
        let dir = fixture("list");
        let all = list(&dir, None);
        assert_eq!(all.len(), 2, "the text file and the data folder are not listed");
        let video = all.iter().find(|e| e.kind == Kind::Video).unwrap();
        assert_eq!((video.name.as_str(), video.duration_ms, video.width, video.height), ("hp_a.mp4", Some(4200), Some(640), Some(480)));
        assert!(video.thumb.is_some());
        let image = all.iter().find(|e| e.kind == Kind::Image).unwrap();
        assert_eq!((image.width, image.height, image.duration_ms), (Some(3), Some(2), None));
        assert!(image.thumb.is_none());
        assert_eq!(list(&dir, Some(Kind::Image)).len(), 1);
        assert!(list(&dir.join("missing"), None).is_empty());
    }

    #[test]
    fn rename_moves_companions_and_cleans_the_name() {
        let dir = fixture("rename");
        let renamed = rename(&dir.join("hp_a.mp4"), "bài: 1").unwrap();
        assert_eq!(renamed, dir.join("bài- 1.mp4"));
        assert!(renamed.exists() && !dir.join("hp_a.mp4").exists());
        assert!(companion_paths(&renamed).iter().all(|p| p.exists()));
        assert!(companion_paths(&dir.join("hp_a.mp4")).iter().all(|p| !p.exists()));
        assert_eq!(list(&dir, Some(Kind::Video))[0].duration_ms, Some(4200));
    }

    #[test]
    fn rename_refuses_an_existing_or_empty_name() {
        let dir = fixture("rename-clash");
        std::fs::write(dir.join("taken.mp4"), b"other").unwrap();
        assert!(rename(&dir.join("hp_a.mp4"), "taken").is_err());
        assert!(rename(&dir.join("hp_a.mp4"), "   ").is_err());
        assert!(dir.join("hp_a.mp4").exists(), "the original is untouched");
        assert_eq!(std::fs::read(dir.join("taken.mp4")).unwrap(), b"other");
    }

    #[test]
    fn renaming_to_the_same_name_changes_nothing() {
        let dir = fixture("rename-same");
        assert_eq!(rename(&dir.join("hp_a.mp4"), "hp_a").unwrap(), dir.join("hp_a.mp4"));
        assert!(dir.join("hp_a.mp4").exists());
    }
}

#[cfg(test)]
mod shared_stem_tests {
    use super::*;

    /// A video and a screenshot with the same name would share `.hpscreen` companion files,
    /// so renaming or deleting one would damage the other's input log.
    #[test]
    fn rename_refuses_a_stem_used_by_the_other_kind() {
        let dir = std::env::temp_dir().join("hp-screen-tests").join(format!("stem-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".hpscreen")).unwrap();
        std::fs::write(dir.join("bai1.mp4"), b"video").unwrap();
        std::fs::write(dir.join("anh.png"), b"image").unwrap();
        std::fs::write(hp_core::library::sidecar_path(&dir.join("bai1.mp4")), b"log").unwrap();

        assert!(rename(&dir.join("anh.png"), "bai1").is_err());
        assert!(dir.join("anh.png").exists());
        assert_eq!(std::fs::read(hp_core::library::sidecar_path(&dir.join("bai1.mp4"))).unwrap(), b"log");
        assert!(stem_in_use(&dir, "BAI1"), "Windows file names ignore case");
        assert!(!stem_in_use(&dir, "bai2"));
    }
}
