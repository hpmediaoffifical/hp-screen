//! Model of the recordings library: which files belong to it, their order, and where the
//! files that accompany a recording live.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Sub-folder of the output folder holding input logs, thumbnails and the index.
pub const DATA_DIR: &str = ".hpscreen";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Video,
    Image,
}

#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Item {
    pub path: PathBuf,
    pub kind: Kind,
    /// Creation time, milliseconds since the Unix epoch.
    pub created_ms: i64,
    pub size: u64,
}

/// `.mp4` is a video and `.png` an image (any letter case); anything else is not ours.
pub fn kind_of(path: &Path) -> Option<Kind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "mp4" => Some(Kind::Video),
        "png" => Some(Kind::Image),
        _ => None,
    }
}

/// Newest first, optionally keeping one kind only.
pub fn sort_and_filter(mut items: Vec<Item>, filter: Option<Kind>) -> Vec<Item> {
    if let Some(kind) = filter {
        items.retain(|i| i.kind == kind);
    }
    items.sort_by(|a, b| b.created_ms.cmp(&a.created_ms).then_with(|| a.path.cmp(&b.path)));
    items
}

/// `<dir>/.hpscreen/<stem>.input.jsonl`
pub fn sidecar_path(media: &Path) -> PathBuf {
    companion(media, "input.jsonl")
}

/// `<dir>/.hpscreen/<stem>.thumb.png`
pub fn thumb_path(media: &Path) -> PathBuf {
    companion(media, "thumb.png")
}

fn companion(media: &Path, suffix: &str) -> PathBuf {
    let stem = media.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let dir = media.parent().unwrap_or_else(|| Path::new(""));
    dir.join(DATA_DIR).join(format!("{stem}.{suffix}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, kind: Kind, created_ms: i64) -> Item {
        Item { path: PathBuf::from(name), kind, created_ms, size: 1 }
    }

    #[test]
    fn kind_is_decided_by_extension_ignoring_case() {
        assert_eq!(kind_of(Path::new("x.MP4")), Some(Kind::Video));
        assert_eq!(kind_of(Path::new("d/hp_a.png")), Some(Kind::Image));
        assert_eq!(kind_of(Path::new("x.txt")), None);
        assert_eq!(kind_of(Path::new("mp4")), None);
        assert_eq!(kind_of(Path::new("hp_a.input.jsonl")), None);
    }

    #[test]
    fn newest_items_come_first_regardless_of_name() {
        // The default name puts the time before the date, so name order is not time order.
        let items = vec![
            item("hp_23-00-00_01-01-2026.mp4", Kind::Video, 1),
            item("hp_01-00-00_03-01-2026.mp4", Kind::Video, 3),
            item("hp_12-00-00_02-01-2026.png", Kind::Image, 2),
        ];
        let sorted = sort_and_filter(items, None);
        let order: Vec<i64> = sorted.iter().map(|i| i.created_ms).collect();
        assert_eq!(order, vec![3, 2, 1]);
    }

    #[test]
    fn filter_keeps_one_kind() {
        let items = vec![item("a.mp4", Kind::Video, 1), item("b.png", Kind::Image, 2), item("c.png", Kind::Image, 3)];
        let images = sort_and_filter(items.clone(), Some(Kind::Image));
        assert_eq!(images.iter().map(|i| i.created_ms).collect::<Vec<_>>(), vec![3, 2]);
        assert_eq!(sort_and_filter(items, Some(Kind::Video)).len(), 1);
    }

    #[test]
    fn companion_files_live_in_the_data_folder() {
        assert_eq!(sidecar_path(Path::new("d/hp_a.mp4")), Path::new("d").join(".hpscreen").join("hp_a.input.jsonl"));
        assert_eq!(thumb_path(Path::new("d/hp_a.mp4")), Path::new("d").join(".hpscreen").join("hp_a.thumb.png"));
        // A stem containing dots keeps everything before the last extension.
        assert_eq!(sidecar_path(Path::new("d/a.b.mp4")), Path::new("d").join(".hpscreen").join("a.b.input.jsonl"));
    }
}

/// `<dir>/.hpscreen/<stem>.meta.json`: duration and size of a recording, written when it ends.
pub fn meta_path(media: &Path) -> PathBuf {
    companion(media, "meta.json")
}

/// Every file that accompanies `media`; they are renamed and deleted together with it.
pub fn companion_paths(media: &Path) -> [PathBuf; 3] {
    [sidecar_path(media), thumb_path(media), meta_path(media)]
}

/// What is known about a recording without opening the video file.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct MediaMeta {
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
}

impl MediaMeta {
    /// Duration from the number of frames written at a constant frame rate.
    pub fn from_frames(frames: u64, fps: u32, width: u32, height: u32) -> Self {
        Self { duration_ms: frames * 1000 / fps.max(1) as u64, width, height }
    }
}

#[cfg(test)]
mod companion_tests {
    use super::*;

    #[test]
    fn meta_file_sits_with_the_other_companions() {
        let data = Path::new("d").join(".hpscreen");
        assert_eq!(meta_path(Path::new("d/hp_a.mp4")), data.join("hp_a.meta.json"));
        assert_eq!(
            companion_paths(Path::new("d/hp_a.mp4")),
            [data.join("hp_a.input.jsonl"), data.join("hp_a.thumb.png"), data.join("hp_a.meta.json")]
        );
    }

    #[test]
    fn duration_comes_from_frame_count_and_rate() {
        assert_eq!(MediaMeta::from_frames(90, 30, 1920, 1080), MediaMeta { duration_ms: 3000, width: 1920, height: 1080 });
        assert_eq!(MediaMeta::from_frames(91, 60, 640, 480).duration_ms, 1516);
        assert_eq!(MediaMeta::from_frames(10, 0, 2, 2).duration_ms, 10_000); // fps 0 is treated as 1
    }
}
