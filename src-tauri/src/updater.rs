//! Self-update from GitHub Releases.
//!
//! A background thread looks for a new release a little after start and every few hours while
//! "Tự động cập nhật" is on. A release found this way is announced to the UI, which asks the
//! user; nothing is installed without a click on "Cập nhật ngay". Installing waits until no
//! recording is in progress, because the installer closes HP Screen. On Windows the installer
//! (passive mode) starts HP Screen again by itself once it is done.
use crate::commands::AppState;
use hp_core::update::{check_due, may_install};
use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

pub const UPDATE_AVAILABLE: &str = "update-available";
pub const UPDATE_PROGRESS: &str = "update-progress";

/// How often the background thread wakes up to see whether a check is due.
const TICK: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct UpdateState {
    /// The newest release found and not yet installed.
    pending: Mutex<Option<Update>>,
    /// A download or install is running.
    installing: AtomicBool,
    last_check: Mutex<Option<Instant>>,
}

/// What the UI shows about a release.
#[derive(Clone, Debug, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum UpdateProgress {
    Downloading { downloaded: u64, total: Option<u64> },
    /// Downloaded; installing starts as soon as the recording ends.
    WaitingForRecording,
    Installing,
    Failed { message: String },
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn info(update: &Update) -> UpdateInfo {
    UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone().filter(|b| !b.trim().is_empty()),
    }
}

fn friendly(e: tauri_plugin_updater::Error) -> String {
    use tauri_plugin_updater::Error as E;
    match e {
        E::Reqwest(_) | E::Network(_) => "Không kết nối được máy chủ cập nhật. Kiểm tra mạng rồi thử lại.".to_string(),
        E::ReleaseNotFound | E::TargetNotFound(_) | E::TargetsNotFound(_) => {
            "Chưa có bản phát hành nào cho Windows trên GitHub.".to_string()
        }
        E::Minisign(_) | E::SignedVersionMismatch { .. } | E::MissingSignedVersion => {
            "Bản cập nhật không có chữ ký hợp lệ nên đã bị từ chối.".to_string()
        }
        other => format!("Cập nhật không thành công: {other}"),
    }
}

fn set_tray_hint(app: &AppHandle, version: Option<&str>) {
    if let Some(tray) = app.tray_by_id("hp-tray") {
        let tip = match version {
            Some(v) => format!("HP Screen — có bản mới {v}"),
            None => "HP Screen".to_string(),
        };
        let _ = tray.set_tooltip(Some(tip));
    }
}

/// Asks GitHub for the newest release; remembers and announces it when it is newer.
async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    let state = app.state::<UpdateState>();
    *lock(&state.last_check) = Some(Instant::now());
    // The installer runs once HP Screen has exited; remove the tray icon so no stale one lingers.
    let handle = app.clone();
    let updater = app
        .updater_builder()
        .on_before_exit(move || {
            let _ = handle.remove_tray_by_id("hp-tray");
        })
        .build()
        .map_err(friendly)?;
    let found = updater.check().await.map_err(friendly)?;
    let result = found.as_ref().map(info);
    set_tray_hint(app, result.as_ref().map(|i| i.version.as_str()));
    *lock(&state.pending) = found;
    if let Some(info) = &result {
        let _ = app.emit(UPDATE_AVAILABLE, info);
    }
    Ok(result)
}

/// Starts the thread behind automatic checks. It reads the setting on every tick, so turning
/// "Tự động cập nhật" on or off takes effect without a restart.
pub fn start_background(app: &AppHandle) {
    let app = app.clone();
    let started = Instant::now();
    std::thread::Builder::new()
        .name("hp-updater".into())
        .spawn(move || loop {
            std::thread::sleep(TICK);
            let auto = app.state::<AppState>().settings().auto_update;
            let state = app.state::<UpdateState>();
            let since_last = lock(&state.last_check).map(|t| t.elapsed().as_secs());
            if state.installing.load(Ordering::SeqCst) || !check_due(auto, started.elapsed().as_secs(), since_last) {
                continue;
            }
            let _result = tauri::async_runtime::block_on(check(&app));
            #[cfg(debug_assertions)]
            eprintln!("[hp] update check: {_result:?}");
        })
        .ok();
}

/// "Kiểm tra cập nhật": looks right now, whatever the automatic setting says.
#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    check(&app).await
}

/// A release found earlier (for a window that opened after it was announced).
#[tauri::command]
pub fn update_pending(state: State<'_, UpdateState>) -> Option<UpdateInfo> {
    lock(&state.pending).as_ref().map(info)
}

/// "Cập nhật ngay": downloads the release, waits for any recording to finish, then installs
/// it. On Windows HP Screen closes here and the installer opens the new version.
#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    let state = app.state::<UpdateState>();
    if state.installing.swap(true, Ordering::SeqCst) {
        return Err("Bản cập nhật đang được tải.".to_string());
    }
    let result = install(&app).await;
    if let Err(message) = &result {
        state.installing.store(false, Ordering::SeqCst);
        let _ = app.emit(UPDATE_PROGRESS, UpdateProgress::Failed { message: message.clone() });
    }
    result
}

async fn install(app: &AppHandle) -> Result<(), String> {
    let pending = lock(&app.state::<UpdateState>().pending).clone();
    let update = match pending {
        Some(update) => update,
        None => {
            check(app).await?;
            lock(&app.state::<UpdateState>().pending).clone().ok_or_else(|| "HP Screen đã là bản mới nhất.".to_string())?
        }
    };

    let progress = |p: UpdateProgress| {
        let _ = app.emit(UPDATE_PROGRESS, p);
    };
    let mut downloaded: u64 = 0;
    let mut last_emit: Option<Instant> = None;
    progress(UpdateProgress::Downloading { downloaded: 0, total: None });
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                if last_emit.is_none_or(|t| t.elapsed() >= Duration::from_millis(150)) {
                    last_emit = Some(Instant::now());
                    progress(UpdateProgress::Downloading { downloaded, total });
                }
            },
            || {},
        )
        .await
        .map_err(friendly)?;

    // Installing closes the app; never cut a recording short or lose one being saved.
    let recorder = app.state::<AppState>().recorder.clone();
    let mut announced = false;
    while !may_install(recorder.status().state) {
        if !announced {
            progress(UpdateProgress::WaitingForRecording);
            announced = true;
        }
        tauri::async_runtime::spawn_blocking(|| std::thread::sleep(Duration::from_secs(2)))
            .await
            .map_err(|e| e.to_string())?;
    }

    progress(UpdateProgress::Installing);
    update.install(bytes).map_err(friendly)?;
    // Only reached where installing does not end the process (not Windows).
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The UI (src/api.ts) switches on `phase`; keep the shape stable.
    #[test]
    fn progress_events_are_tagged_by_phase() {
        let v = |p| serde_json::to_value(p).unwrap();
        assert_eq!(
            v(UpdateProgress::Downloading { downloaded: 5, total: Some(10) }),
            json!({"phase":"downloading","downloaded":5,"total":10})
        );
        assert_eq!(v(UpdateProgress::WaitingForRecording), json!({"phase":"waiting_for_recording"}));
        assert_eq!(v(UpdateProgress::Installing), json!({"phase":"installing"}));
        assert_eq!(v(UpdateProgress::Failed { message: "x".into() }), json!({"phase":"failed","message":"x"}));
    }
}
