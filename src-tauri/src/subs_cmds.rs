//! Tauri commands for subtitles: install the recogniser (with consent, from the UI), make
//! subtitles from a video's sound, and write an SRT next to the video.
use crate::asr::{self, CLI, MODELS, Model};
use crate::commands::{AppState, library_item};
use hp_core::edit::EditDoc;
use hp_core::subtitle::{Cue, cues_from_segments, parse_whisper_json, to_srt};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

pub const ASR_PROGRESS: &str = "asr-progress";
pub const ASR_DONE: &str = "asr-done";
pub const ASR_ERROR: &str = "asr-error";
pub const SUBS_PROGRESS: &str = "subs-progress";
pub const SUBS_DONE: &str = "subs-done";
pub const SUBS_ERROR: &str = "subs-error";

/// The install or recognition running now (one at a time), by its cancel flag.
#[derive(Default)]
pub struct AsrState {
    job: Mutex<Option<Arc<AtomicBool>>>,
}

#[derive(Clone, Serialize)]
pub struct ModelStatus {
    pub id: &'static str,
    pub name: &'static str,
    pub size_mb: u64,
    pub installed: bool,
}

#[derive(Clone, Serialize)]
pub struct AsrStatus {
    pub cli: bool,
    pub cli_size_mb: u64,
    pub models: Vec<ModelStatus>,
    pub folder: String,
}

#[derive(Clone, Serialize)]
struct Progress {
    phase: &'static str,
    done: u64,
    total: u64,
}

#[derive(Clone, Serialize)]
struct Message {
    message: String,
}

#[derive(Clone, Serialize)]
struct SubsProgress {
    source: String,
    phase: &'static str,
    percent: u32,
}

#[derive(Clone, Serialize)]
struct SubsDone {
    source: String,
    cues: Vec<Cue>,
}

#[derive(Clone, Serialize)]
struct SubsError {
    source: String,
    message: String,
}

/// What the user is shown of an error: its first line. The rest (whisper's output) goes to the log.
fn user_message(message: &str) -> &str {
    message.lines().next().unwrap_or(message)
}

fn mb(bytes: u64) -> u64 {
    bytes.div_ceil(1_000_000)
}

/// A model counts as installed when its file is there with the right size (the SHA-256 was
/// checked when it was downloaded).
fn model_installed(dir: &Path, m: &Model) -> bool {
    std::fs::metadata(asr::model_path(dir, m)).is_ok_and(|md| md.len() == m.download.bytes)
}

fn status_in(dir: &Path) -> AsrStatus {
    AsrStatus {
        cli: asr::find_cli(&dir.join("bin")).is_some(),
        cli_size_mb: mb(CLI.bytes),
        models: MODELS
            .iter()
            .map(|m| ModelStatus { id: m.id, name: m.name, size_mb: mb(m.download.bytes), installed: model_installed(dir, m) })
            .collect(),
        folder: dir.to_string_lossy().into_owned(),
    }
}

/// Starts a job unless one runs; returns its cancel flag.
fn begin(state: &AsrState) -> Result<Arc<AtomicBool>, String> {
    let mut job = state.job.lock().unwrap_or_else(|e| e.into_inner());
    if job.is_some() {
        return Err("Đang tải bộ nhận giọng hoặc đang tạo phụ đề, hãy đợi xong".to_string());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    *job = Some(cancel.clone());
    Ok(cancel)
}

fn end(app: &AppHandle) {
    if let Some(s) = app.try_state::<AsrState>() {
        *s.job.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

#[tauri::command]
pub fn asr_status() -> AsrStatus {
    status_in(&asr::asr_dir())
}

/// Downloads what is missing for `model` (the program and/or the model). The UI calls this
/// only after the user agreed in its dialog.
#[tauri::command]
pub fn asr_install(app: AppHandle, state: State<'_, AsrState>, model: String) -> Result<(), String> {
    let m = *asr::model(&model).ok_or("Model không có trong danh sách")?;
    let cancel = begin(&state)?;
    std::thread::spawn(move || {
        let dir = asr::asr_dir();
        let result = (|| -> Result<(), String> {
            if asr::find_cli(&dir.join("bin")).is_none() {
                let mut report = |done, total| {
                    let _ = app.emit(ASR_PROGRESS, Progress { phase: "cli", done, total });
                };
                let zip = asr::download(&CLI, &dir, &mut report, &cancel)?;
                let installed = asr::install_cli(&zip, &dir);
                let _ = std::fs::remove_file(&zip);
                installed?;
            }
            if !model_installed(&dir, &m) {
                let mut report = |done, total| {
                    let _ = app.emit(ASR_PROGRESS, Progress { phase: "model", done, total });
                };
                asr::download(&m.download, &dir, &mut report, &cancel)?;
            }
            Ok(())
        })();
        end(&app);
        match result {
            Ok(()) => {
                let _ = app.emit(ASR_DONE, ());
            }
            Err(message) => {
                if !cancel.load(Ordering::Relaxed) {
                    crate::paths::log_error(&app, &format!("Tải bộ nhận giọng thất bại: {message}"));
                }
                let message = user_message(&message).to_string();
                let _ = app.emit(ASR_ERROR, Message { message });
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub fn asr_cancel(state: State<'_, AsrState>) {
    if let Some(c) = state.job.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        c.store(true, Ordering::Relaxed);
    }
}

/// Recognises the speech of `path` with `model` and sends the cues back (`subs-done`).
#[tauri::command]
pub fn subtitles_generate(app: AppHandle, state: State<'_, AppState>, asr_state: State<'_, AsrState>, path: String, model: String) -> Result<(), String> {
    let media = library_item(&app, &state, &path)?;
    let m = *asr::model(&model).ok_or("Model không có trong danh sách")?;
    let dir = asr::asr_dir();
    let cli = asr::find_cli(&dir.join("bin")).ok_or("Chưa tải bộ nhận giọng")?;
    if !model_installed(&dir, &m) {
        return Err("Chưa tải model nhận giọng".to_string());
    }
    let cancel = begin(&asr_state)?;
    std::thread::spawn(move || {
        let source = path;
        let work = std::env::temp_dir().join("hp-screen-asr");
        let wav = work.join(format!("{}.wav", std::process::id()));
        let result = (|| -> Result<Vec<Cue>, String> {
            std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
            let _ = app.emit(SUBS_PROGRESS, SubsProgress { source: source.clone(), phase: "audio", percent: 0 });
            if !asr::write_wav_16k_mono(&media, &wav, &cancel)? {
                return Err("Video này không có tiếng".to_string());
            }
            let mut report = |percent| {
                let _ = app.emit(SUBS_PROGRESS, SubsProgress { source: source.clone(), phase: "recognise", percent });
            };
            let json = asr::run_whisper(&cli, &asr::model_path(&dir, &m), &wav, &mut report, &cancel)?;
            Ok(cues_from_segments(&parse_whisper_json(&json)?))
        })();
        let _ = std::fs::remove_file(&wav);
        end(&app);
        match result {
            Ok(cues) => {
                let _ = app.emit(SUBS_DONE, SubsDone { source, cues });
            }
            Err(message) => {
                if !cancel.load(Ordering::Relaxed) {
                    crate::paths::log_error(&app, &format!("Tạo phụ đề thất bại: {message}"));
                }
                let message = user_message(&message).to_string();
                let _ = app.emit(SUBS_ERROR, SubsError { source, message });
            }
        }
    });
    Ok(())
}

/// Writes the subtitles of `doc` as `<name>_edit.srt` next to the video (never over a file).
fn write_srt(media: &Path, doc: &EditDoc, duration_ms: i64) -> Result<PathBuf, String> {
    let srt = to_srt(&doc.subtitles.cues, &doc.cuts(duration_ms));
    if srt.is_empty() {
        return Err("Chưa có dòng phụ đề nào để xuất".to_string());
    }
    let out = crate::export::export_path_ext(media, "srt");
    // A UTF-8 BOM so older players read Vietnamese correctly.
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(srt.replace('\n', "\r\n").as_bytes());
    std::fs::write(&out, bytes).map_err(|e| format!("Không ghi được file SRT: {e}"))?;
    Ok(out)
}

#[tauri::command]
pub fn export_srt(app: AppHandle, state: State<'_, AppState>, path: String, doc: EditDoc, duration_ms: i64) -> Result<String, String> {
    let media = library_item(&app, &state, &path)?;
    write_srt(&media, &doc, duration_ms).map(|p| p.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_media::temp_path;
    use hp_core::cut::Span;

    #[test]
    fn status_reports_what_is_missing() {
        let dir = temp_path("asr-status", "x").parent().unwrap().to_path_buf();
        let s = status_in(&dir);
        assert!(!s.cli && s.models.iter().all(|m| !m.installed));
        assert_eq!(s.models.len(), MODELS.len());
        assert_eq!(s.models[0].size_mb, 575);
    }

    #[test]
    fn srt_is_written_next_to_the_video_without_overwriting() {
        let media = temp_path("srt-write", "bài.mp4");
        let mut doc = EditDoc::default();
        doc.subtitles.cues = vec![Cue { start_ms: 0, end_ms: 1000, text: "Xin chào".into() }];
        doc.cuts = vec![Span { start_ms: 5000, end_ms: 6000 }];
        let a = write_srt(&media, &doc, 10_000).unwrap();
        assert_eq!(a, media.with_file_name("bài_edit.srt"));
        let text = std::fs::read(&a).unwrap();
        assert_eq!(&text[..3], &[0xEF, 0xBB, 0xBF]);
        assert!(String::from_utf8_lossy(&text[3..]).contains("00:00:00,000 --> 00:00:01,000\r\nXin chào"));
        let b = write_srt(&media, &doc, 10_000).unwrap();
        assert_eq!(b, media.with_file_name("bài_edit_2.srt"));
        assert!(write_srt(&media, &EditDoc::default(), 10_000).is_err(), "nothing to write");
    }

    #[test]
    fn the_user_sees_one_line_and_the_log_gets_the_details() {
        let full = "Không tạo được phụ đề (mã lỗi 0x3).
whisper_init: failed
error: model";
        assert_eq!(user_message(full), "Không tạo được phụ đề (mã lỗi 0x3).");
        assert_eq!(user_message("Đã hủy"), "Đã hủy");
    }

    #[test]
    fn only_one_job_at_a_time() {
        let s = AsrState::default();
        let first = begin(&s).unwrap();
        assert!(begin(&s).is_err());
        first.store(true, Ordering::Relaxed);
    }
}
