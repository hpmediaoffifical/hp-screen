//! Tauri commands of the edit page: open and save an edit, and run one export at a time.
use crate::commands::{AppState, library_item};
use crate::decoder::{VideoDecoder, VideoInfo};
use crate::export::{export_path, run_export};
use hp_core::cut::Span;
use hp_core::edit::{EditDoc, LoadError, load_edit, save_edit};
use hp_core::effects::{Effects, build_effects};
use hp_core::zoom::{Camera, ZoomSegment, auto_segments, camera_track, regenerate};
use hp_core::input_log::parse_log;
use hp_core::library::{DATA_DIR, Kind, edit_path, kind_of, sidecar_path};
use hp_core::plan::{Plan, Source};
use hp_core::state::RecState;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

pub const EXPORT_PROGRESS: &str = "export-progress";
pub const EXPORT_DONE: &str = "export-done";
pub const EXPORT_ERROR: &str = "export-error";

/// The export running now, if any: its cancel flag.
#[derive(Default)]
pub struct ExportState {
    running: Mutex<Option<Arc<AtomicBool>>>,
}

#[derive(Serialize)]
pub struct EditSession {
    pub info: VideoInfo,
    pub doc: EditDoc,
    pub has_log: bool,
    /// Click and key effects from the input log, for the preview.
    pub effects: Option<Effects>,
    pub warning: Option<String>,
}

#[derive(Serialize)]
pub struct KeepInfo {
    pub keep: Vec<Span>,
    pub output_ms: i64,
    pub can_export: bool,
}

// Every export event names its source video, so an edit page only follows its own export.
#[derive(Clone, Serialize)]
struct Progress {
    source: String,
    done: u64,
    total: u64,
}

#[derive(Clone, Serialize)]
struct Done {
    source: String,
    path: String,
}

#[derive(Clone, Serialize)]
struct Failed {
    source: String,
    message: String,
}

/// Reads the edit of `media`. A damaged file gives a fresh edit and a warning; a file from a
/// newer HP Screen is refused so it is never overwritten.
fn load_doc(media: &Path) -> Result<(EditDoc, Option<String>), String> {
    let Ok(text) = std::fs::read_to_string(edit_path(media)) else {
        return Ok((EditDoc::default(), None));
    };
    match load_edit(&text) {
        Ok(doc) => Ok((doc, None)),
        Err(LoadError::Newer(_)) => Err("File chỉnh sửa do bản HP Screen mới hơn tạo".to_string()),
        Err(LoadError::Invalid) => Ok((EditDoc::default(), Some("File chỉnh sửa bị hỏng, bắt đầu lại từ đầu".to_string()))),
    }
}

/// Writes the edit of `media` through a temporary file, so a crash never leaves half a file.
fn save_doc(media: &Path, doc: &EditDoc) -> Result<(), String> {
    let path = edit_path(media);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Không lưu được chỉnh sửa: {e}"))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, save_edit(doc)).map_err(|e| format!("Không lưu được chỉnh sửa: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Không lưu được chỉnh sửa: {e}")
    })
}

fn video_item(app: &AppHandle, state: &State<'_, AppState>, path: &str) -> Result<PathBuf, String> {
    let media = library_item(app, state, path)?;
    if kind_of(&media) != Some(Kind::Video) {
        return Err("Chỉ chỉnh sửa được video".to_string());
    }
    Ok(media)
}

fn open_video(media: &Path) -> Result<VideoInfo, String> {
    VideoDecoder::open(media).map(|(_, info)| info).map_err(|_| "Không đọc được video này".to_string())
}

fn plan_for(info: &VideoInfo, doc: &EditDoc, effects: Option<Effects>) -> Plan {
    let src = Source { width: info.width, height: info.height, duration_ms: info.duration_ms, fps: info.fps };
    Plan::new(src, doc, effects)
}

/// Effects from the input log next to `media`, or `None` when it has no (readable) log.
fn effects_for(media: &Path, info: &VideoInfo) -> Option<Effects> {
    let text = std::fs::read_to_string(sidecar_path(media)).ok()?;
    let log = parse_log(&text);
    if log.events.is_empty() {
        return None;
    }
    Some(build_effects(&log, info.width, info.height))
}

/// Zoom segments made from the clicks in `effects`, keeping manual ones; without a log the
/// existing segments stay as they are.
fn auto_zoom(effects: Option<&Effects>, doc: &EditDoc, w: u32, h: u32, duration_ms: i64) -> Vec<ZoomSegment> {
    let Some(fx) = effects else { return doc.zoom.segments.clone() };
    let (w, h) = (w.max(1) as f32, h.max(1) as f32);
    let clicks: Vec<(i64, f32, f32)> = fx.ripples.iter().map(|r| (r.t_ms, r.x / w, r.y / h)).collect();
    regenerate(&doc.zoom.segments, auto_segments(&clicks, duration_ms))
}

/// Camera samples for the preview: every 1000/60 ms, as in the export.
fn zoom_samples(effects: Option<&Effects>, doc: &EditDoc, w: u32, h: u32, duration_ms: i64) -> Vec<(i64, Camera)> {
    let (fw, fh) = (w.max(1) as f32, h.max(1) as f32);
    let cursor = |t: i64| effects.and_then(|e| e.cursor_at(t)).map(|(x, y)| (x / fw, y / fh));
    camera_track(&doc.zoom, &cursor, duration_ms, 1000 / 60)
}

/// The video's size and length as the edit page knows them (saves reopening the file).
#[derive(serde::Deserialize)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub duration_ms: i64,
}

fn frame_info(f: &Frame) -> VideoInfo {
    VideoInfo { width: f.width, height: f.height, duration_ms: f.duration_ms, fps: 30, has_audio: false }
}

/// Automatic zoom segments from the clicks in the input log, keeping manual ones.
#[tauri::command]
pub async fn zoom_auto(app: AppHandle, state: State<'_, AppState>, path: String, doc: EditDoc, frame: Frame) -> Result<Vec<ZoomSegment>, String> {
    let media = video_item(&app, &state, &path)?;
    let fx = effects_for(&media, &frame_info(&frame));
    Ok(auto_zoom(fx.as_ref(), &doc, frame.width, frame.height, frame.duration_ms))
}

/// The zoom camera over source time, for the preview.
#[tauri::command]
pub async fn zoom_track(app: AppHandle, state: State<'_, AppState>, path: String, doc: EditDoc, frame: Frame) -> Result<Vec<(i64, Camera)>, String> {
    let media = video_item(&app, &state, &path)?;
    let fx = effects_for(&media, &frame_info(&frame));
    Ok(zoom_samples(fx.as_ref(), &doc, frame.width, frame.height, frame.duration_ms))
}

/// Removes `.part` files an export left behind when the app was closed while exporting.
/// Called at start-up and before each export (never while one is running).
pub(crate) fn remove_stale_parts(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir.join(DATA_DIR)) else { return };
    for e in entries.flatten() {
        if e.path().extension().is_some_and(|x| x == "part") {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Async: opening a long video may take a moment, and must not block the UI thread.
#[tauri::command]
pub async fn edit_open(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<EditSession, String> {
    let media = video_item(&app, &state, &path)?;
    let info = open_video(&media)?;
    let (doc, warning) = load_doc(&media)?;
    let effects = effects_for(&media, &info);
    Ok(EditSession { info, doc, has_log: sidecar_path(&media).is_file(), effects, warning })
}

#[tauri::command]
pub fn edit_save(app: AppHandle, state: State<'_, AppState>, path: String, doc: EditDoc) -> Result<(), String> {
    let media = video_item(&app, &state, &path)?;
    save_doc(&media, &doc)
}

/// What an edit keeps of a video of `duration_ms`.
#[tauri::command]
pub fn edit_keep(doc: EditDoc, duration_ms: i64) -> KeepInfo {
    let cuts = doc.cuts(duration_ms);
    KeepInfo { keep: cuts.keep().to_vec(), output_ms: cuts.output_ms(), can_export: cuts.output_ms() >= hp_core::cut::MIN_OUTPUT_MS }
}

/// Starts exporting `doc` of `path` in the background; returns the file it will write.
#[tauri::command]
pub async fn export_start(
    app: AppHandle,
    state: State<'_, AppState>,
    exports: State<'_, ExportState>,
    path: String,
    doc: EditDoc,
) -> Result<String, String> {
    let media = video_item(&app, &state, &path)?;
    if state.recorder.status().state != RecState::Idle {
        return Err("Đang quay, hãy xuất sau khi dừng".to_string());
    }
    let mut running = exports.running.lock().unwrap_or_else(|e| e.into_inner());
    if running.is_some() {
        return Err("Đang xuất một video khác".to_string());
    }
    let info = open_video(&media)?;
    let plan = plan_for(&info, &doc, effects_for(&media, &info));
    if !plan.can_export() {
        return Err("Phần giữ lại quá ngắn để xuất".to_string());
    }
    if let Some(dir) = media.parent() {
        remove_stale_parts(dir);
    }
    let out = export_path(&media);
    let (w, h) = plan.out_size();
    let bitrate = hp_core::quality::video_bitrate(w, h, plan.fps(), state.settings().quality);
    let cancel = Arc::new(AtomicBool::new(false));
    *running = Some(cancel.clone());
    drop(running);
    let out_name = out.to_string_lossy().into_owned();
    std::thread::spawn(move || {
        let source = path;
        let mut report = |done, total| {
            let _ = app.emit(EXPORT_PROGRESS, Progress { source: source.clone(), done, total });
        };
        let result = run_export(&media, &out, &plan, bitrate, &mut report, &cancel);
        if let Some(exports) = tauri::Manager::try_state::<ExportState>(&app) {
            *exports.running.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
        match result {
            Ok(written) => {
                let _ = app.emit(EXPORT_DONE, Done { source, path: written.to_string_lossy().into_owned() });
                crate::events::library_changed(&app);
            }
            Err(message) => {
                if !cancel.load(Ordering::Relaxed) {
                    crate::paths::log_error(&app, &format!("Xuất video thất bại: {message}"));
                }
                let _ = app.emit(EXPORT_ERROR, Failed { source, message });
            }
        }
    });
    Ok(out_name)
}

#[tauri::command]
pub fn export_cancel(exports: State<'_, ExportState>) {
    if let Some(cancel) = exports.running.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_media::temp_path;
    use hp_core::cut::Span;

    fn info() -> VideoInfo {
        VideoInfo { width: 640, height: 480, duration_ms: 1000, fps: 30, has_audio: false }
    }

    #[test]
    fn zoom_auto_without_log_keeps_segments() {
        use hp_core::zoom::ZoomSegment;
        let mut doc = EditDoc::default();
        doc.zoom.segments = vec![ZoomSegment { start_ms: 0, end_ms: 2000, manual: true, ..ZoomSegment::default() }];
        assert_eq!(auto_zoom(None, &doc, 640, 480, 10_000), doc.zoom.segments);
    }

    #[test]
    fn zoom_auto_makes_segments_from_clicks() {
        use hp_core::input_log::{Button, Event, ParsedLog};
        let log = ParsedLog { header: None, events: vec![Event::Down { t: 1000, b: Button::Left, x: 160, y: 120 }] };
        let fx = build_effects(&log, 640, 480);
        let out = auto_zoom(Some(&fx), &EditDoc::default(), 640, 480, 10_000);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].center, Some((0.25, 0.25)), "clicks are normalised to the frame");
    }

    #[test]
    fn effects_come_from_the_input_log() {
        let media = temp_path("edit-log", "a.mp4");
        assert_eq!(effects_for(&media, &info()), None);
        let log = sidecar_path(&media);
        std::fs::create_dir_all(log.parent().unwrap()).unwrap();
        std::fs::write(&log, "{\"e\":\"down\",\"t\":100,\"b\":\"right\",\"x\":5,\"y\":6}
").unwrap();
        let fx = effects_for(&media, &info()).expect("effects from the log");
        assert_eq!(fx.ripples.len(), 1);
    }

    #[test]
    fn stale_parts_are_removed_and_nothing_else() {
        let media = temp_path("edit-parts", "a.mp4");
        let dir = media.parent().unwrap();
        let data = dir.join(DATA_DIR);
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(data.join("a_edit.mp4.part"), b"half").unwrap();
        std::fs::write(data.join("a.edit.json"), b"{}").unwrap();
        std::fs::write(dir.join("b.part"), b"user file").unwrap();
        remove_stale_parts(dir);
        assert!(!data.join("a_edit.mp4.part").exists());
        assert!(data.join("a.edit.json").exists());
        assert!(dir.join("b.part").exists(), "only the data folder is cleaned");
    }

    #[test]
    fn missing_file_gives_a_fresh_edit() {
        let media = temp_path("edit-missing", "a.mp4");
        assert_eq!(load_doc(&media).unwrap(), (EditDoc::default(), None));
    }

    #[test]
    fn save_then_load_round_trips() {
        let media = temp_path("edit-roundtrip", "a.mp4");
        let doc = EditDoc { cuts: vec![Span { start_ms: 100, end_ms: 900 }], ..EditDoc::default() };
        save_doc(&media, &doc).unwrap();
        assert_eq!(load_doc(&media).unwrap(), (doc, None));
    }

    #[test]
    fn corrupt_file_gives_defaults_with_warning() {
        let media = temp_path("edit-corrupt", "a.mp4");
        std::fs::create_dir_all(edit_path(&media).parent().unwrap()).unwrap();
        std::fs::write(edit_path(&media), "{oops").unwrap();
        let (doc, warning) = load_doc(&media).unwrap();
        assert_eq!(doc, EditDoc::default());
        assert_eq!(warning.as_deref(), Some("File chỉnh sửa bị hỏng, bắt đầu lại từ đầu"));
    }

    #[test]
    fn newer_file_is_refused() {
        let media = temp_path("edit-newer", "a.mp4");
        std::fs::create_dir_all(edit_path(&media).parent().unwrap()).unwrap();
        std::fs::write(edit_path(&media), "{\"v\":9}").unwrap();
        assert_eq!(load_doc(&media).unwrap_err(), "File chỉnh sửa do bản HP Screen mới hơn tạo");
    }

    #[test]
    fn save_leaves_no_tmp_file() {
        let media = temp_path("edit-tmp", "a.mp4");
        save_doc(&media, &EditDoc::default()).unwrap();
        save_doc(&media, &EditDoc::default()).unwrap();
        let dir = edit_path(&media).parent().unwrap().to_path_buf();
        let names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["a.edit.json".to_string()]);
    }
}
