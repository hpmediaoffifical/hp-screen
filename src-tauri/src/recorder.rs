//! The recording session as the UI sees it: the state machine from `hp_core::state` driving a
//! `Pipeline`, with every change reported to the UI as a `rec-state` event.
use crate::capture::{self, Target};
use crate::input_hook::{InputHook, start_input_hook};
use crate::live_layer::LiveLayer;
use crate::events;
use crate::paths::{new_media_path, output_dir};
use crate::pipeline::{EventFn, Pipeline, PipelineConfig, PipelineEvent};
use hp_core::input_log::{Header, LOG_VERSION};
use hp_core::library::{sidecar_path, thumb_path};
use hp_core::settings::Settings;
use hp_core::state::{Command, RecState, pause_toggle_command, toggle_command, transition};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tauri::AppHandle;

/// Snapshot sent to the UI in `rec-state` events and returned by `rec_status`.
#[derive(Clone, Debug, Serialize)]
pub struct RecStatus {
    pub state: RecState,
    pub elapsed_ms: i64,
    /// Seconds left before recording starts, while counting down.
    pub countdown: Option<u32>,
    pub file: Option<String>,
    pub warning: Option<String>,
}

#[derive(Clone)]
pub struct Recorder {
    inner: Arc<Inner>,
}

struct Inner {
    host: Arc<dyn Host>,
    state: Mutex<RecState>,
    pipeline: Mutex<Option<Pipeline>>,
    input_hook: Mutex<Option<InputHook>>,
    file: Mutex<Option<PathBuf>>,
    last_target: Mutex<Option<Target>>,
    elapsed_ms: AtomicI64,
    /// Bumped on every start so a cancelled countdown cannot start a later session.
    generation: AtomicU64,
    /// Strokes and webcam drawn into the recording.
    live: Arc<LiveLayer>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Recorder {
    pub fn new(app: AppHandle) -> Self {
        Self::with_host(Arc::new(AppHost(app)))
    }

    pub fn with_host(host: Arc<dyn Host>) -> Self {
        Self {
            inner: Arc::new(Inner {
                host,
                state: Mutex::new(RecState::Idle),
                pipeline: Mutex::new(None),
                input_hook: Mutex::new(None),
                file: Mutex::new(None),
                last_target: Mutex::new(None),
                elapsed_ms: AtomicI64::new(0),
                generation: AtomicU64::new(0),
                live: LiveLayer::new(),
            }),
        }
    }

    pub fn status(&self) -> RecStatus {
        self.inner.status(None, None)
    }

    /// The layer drawn into recorded frames (strokes, webcam).
    pub fn live(&self) -> Arc<LiveLayer> {
        self.inner.live.clone()
    }

    /// What the hotkeys and the tray record: the last selection made in the UI or last recorded.
    pub fn last_target(&self) -> Option<Target> {
        lock(&self.inner.last_target).clone()
    }

    /// Remembers the UI selection so hotkeys and the tray act on it.
    pub fn set_target(&self, target: Target) {
        *lock(&self.inner.last_target) = Some(target);
    }

    #[cfg(test)]
    fn has_pipeline(&self) -> bool {
        lock(&self.inner.pipeline).is_some() || lock(&self.inner.input_hook).is_some()
    }

    /// Starts the countdown and then the recording. Returns as soon as the countdown begins.
    pub fn start(&self, target: Target, settings: Settings) -> Result<(), String> {
        self.inner.apply(Command::Start)?;
        *lock(&self.inner.last_target) = Some(target.clone());
        self.inner.elapsed_ms.store(0, Ordering::SeqCst);
        let generation = self.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let inner = self.inner.clone();
        std::thread::spawn(move || inner.count_down_and_begin(generation, target, settings));
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), String> {
        self.inner.apply(Command::Cancel)?;
        self.inner.reset_live();
        self.inner.emit(None, None);
        Ok(())
    }

    pub fn toggle_pause(&self) -> Result<(), String> {
        let state = *lock(&self.inner.state);
        let command = pause_toggle_command(state).ok_or("Chỉ tạm dừng được khi đang quay")?;
        self.inner.apply(command)?;
        if let Some(pipeline) = lock(&self.inner.pipeline).as_ref() {
            match command {
                Command::Pause => pipeline.pause(),
                _ => pipeline.resume(),
            }
        }
        self.inner.emit(None, None);
        Ok(())
    }

    /// Stops recording; the file is closed in the background and `rec-state` reports Idle after.
    pub fn stop(&self) -> Result<(), String> {
        self.inner.clone().stop()
    }

    /// The start/stop hotkey: start, cancel the countdown, or stop, depending on the state.
    pub fn toggle(&self, default_target: Target, settings: Settings) -> Result<(), String> {
        let state = *lock(&self.inner.state);
        match toggle_command(state) {
            Some(Command::Start) => self.start(self.last_target().unwrap_or(default_target), settings),
            Some(Command::Cancel) => self.cancel(),
            Some(Command::Stop) => self.stop(),
            _ => Ok(()), // still closing the previous file: ignore the key press
        }
    }
}

impl Inner {
    /// Nothing drawn in one session carries over to the next.
    fn reset_live(&self) {
        self.live.set_drawing(false);
        self.live.clear();
        self.live.set_camera(None);
    }

    fn apply(&self, command: Command) -> Result<RecState, String> {
        let mut state = lock(&self.state);
        let next = transition(*state, command).map_err(|_| "Thao tác này không dùng được ở trạng thái hiện tại".to_string())?;
        *state = next;
        Ok(next)
    }

    fn status(&self, countdown: Option<u32>, warning: Option<String>) -> RecStatus {
        RecStatus {
            state: *lock(&self.state),
            elapsed_ms: self.elapsed_ms.load(Ordering::SeqCst),
            countdown,
            file: lock(&self.file).as_ref().map(|p| p.to_string_lossy().into_owned()),
            warning,
        }
    }

    fn emit(&self, countdown: Option<u32>, warning: Option<String>) {
        self.host.state(&self.status(countdown, warning));
    }

    fn count_down_and_begin(self: Arc<Self>, generation: u64, target: Target, settings: Settings) {
        let still_ours = |inner: &Inner| {
            *lock(&inner.state) == RecState::Countdown && inner.generation.load(Ordering::SeqCst) == generation
        };
        for remaining in (1..=settings.countdown_secs).rev() {
            self.emit(Some(remaining), None);
            for _ in 0..10 {
                std::thread::sleep(Duration::from_millis(100));
                if !still_ours(&self) {
                    return; // cancelled
                }
            }
        }
        // Held from before the state says Recording until the pipeline is stored. A stop, pause
        // or failure arriving while the recording starts up waits here and then finds the real
        // pipeline, instead of finding nothing and leaving a recording running behind an idle UI.
        let mut slot = lock(&self.pipeline);
        if !still_ours(&self) || self.apply(Command::CountdownDone).is_err() {
            return;
        }
        match self.clone().begin(target, &settings) {
            Ok(pipeline) => *slot = Some(pipeline),
            Err(message) => {
                // If a stop already arrived, its thread finishes the session; otherwise end it here.
                let mut state = lock(&self.state);
                if matches!(*state, RecState::Recording | RecState::Paused) {
                    *state = RecState::Idle;
                }
                drop(state);
                *lock(&self.file) = None;
                self.host.error(&message);
            }
        }
        drop(slot);
        self.emit(None, None);
    }

    /// Starts capturing. The caller holds the pipeline slot and stores the returned pipeline.
    fn begin(self: Arc<Self>, target: Target, settings: &Settings) -> Result<Pipeline, String> {
        let dir = self.host.output_dir(settings)?;
        crate::paths::ensure_recordable(&dir)?;
        let space_dir = dir.clone();
        let ticks = std::sync::atomic::AtomicU32::new(0);
        let path = new_media_path(&dir, settings, "mp4");
        let log_header = if settings.input_log {
            capture::target_rect(&target).ok().map(|region| Header {
                v: LOG_VERSION,
                mode: target.mode_name().to_string(),
                region,
                width: region.w & !1,
                height: region.h & !1,
                fps: settings.fps,
                // A smooth cursor needs the log: it is only drawn when the log is kept.
                cursor: settings.smooth_cursor,
            })
        } else {
            None
        };
        let mut cfg = PipelineConfig {
            target,
            path: path.clone(),
            thumb: Some(thumb_path(&path)),
            fps: settings.fps,
            quality: settings.quality,
            audio: (settings.system_audio || settings.mic)
                .then_some(crate::audio::AudioConfig { system: settings.system_audio, mic: settings.mic }),
            live: Some(self.live.clone()),
            cursor: true,
            timeline: None,
        };
        self.reset_live();
        self.live.set_hold_ms(settings.draw_fade_secs as i64 * 1000);
        let weak = Arc::downgrade(&self);
        let on_event: EventFn = Arc::new(move |event| {
            let Some(inner) = weak.upgrade() else { return };
            match event {
                PipelineEvent::Tick { elapsed_ms, warning } => {
                    inner.elapsed_ms.store(elapsed_ms, Ordering::SeqCst);
                    inner.emit(None, warning);
                    // About every five seconds: stop while there is still room to close the file.
                    let low = ticks.fetch_add(1, Ordering::SeqCst) % 10 == 9
                        && crate::paths::free_space(&space_dir).is_some_and(|free| free < crate::paths::MIN_FREE_BYTES);
                    if low {
                        inner.host.error("Ổ đĩa sắp đầy (còn dưới 500 MB) nên HP Screen đã dừng quay và lưu video.");
                        std::thread::spawn(move || {
                            let _ = inner.stop();
                        });
                    }
                }
                // Both arrive on pipeline threads; stopping joins those threads, so hand over.
                PipelineEvent::SourceClosed => {
                    std::thread::spawn(move || {
                        let _ = inner.stop();
                    });
                }
                PipelineEvent::Failed(message) => {
                    std::thread::spawn(move || inner.fail(&message));
                }
            }
        });
        let own_hotkeys =
            vec![
            settings.hotkey_toggle.clone(),
            settings.hotkey_pause.clone(),
            settings.hotkey_screenshot.clone(),
            settings.hotkey_quickshot.clone(),
            settings.hotkey_draw.clone(),
        ];
        // The input log starts first, on the recording's clock. It is an extra: failing to start
        // it must not stop the recording, but then the Windows cursor is recorded as usual
        // (a smooth cursor cannot be drawn without the log).
        let timeline = crate::pipeline::waiting_timeline();
        let hook = log_header.and_then(|header| start_input_hook(&sidecar_path(&path), header, timeline.clone(), own_hotkeys, self.live.drawing_flag()).ok());
        cfg.cursor = !(settings.smooth_cursor && hook.is_some());
        cfg.timeline = Some(timeline);
        let pipeline = match Pipeline::start(cfg, on_event) {
            Ok(p) => p,
            Err(e) => {
                drop(hook);
                let _ = std::fs::remove_file(sidecar_path(&path));
                return Err(e);
            }
        };
        *lock(&self.input_hook) = hook;
        *lock(&self.file) = Some(path);
        Ok(pipeline)
    }

    fn stop(self: Arc<Self>) -> Result<(), String> {
        self.apply(Command::Stop)?;
        self.emit(None, None);
        std::thread::spawn(move || {
            // Take the pipeline first: this waits for a recording that is still starting up,
            // so the hooks it installs are the ones removed on the next line.
            let pipeline = lock(&self.pipeline).take();
            lock(&self.input_hook).take(); // remove the hooks as soon as recording ends
            let result = match pipeline {
                Some(pipeline) => pipeline.finish().map(|stats| {
                    if let Some(path) = lock(&self.file).clone() {
                        let meta = hp_core::library::MediaMeta::from_frames(stats.frames, stats.fps, stats.width, stats.height);
                        crate::library_fs::write_meta(&path, &meta);
                    }
                }),
                None => Ok(()),
            };
            self.finalized(result.err());
        });
        Ok(())
    }

    /// The pipeline reported an unrecoverable error while recording.
    fn fail(self: Arc<Self>, message: &str) {
        let pipeline = lock(&self.pipeline).take();
        lock(&self.input_hook).take();
        if pipeline.is_none() {
            return; // a stop is already closing the file and will report the outcome
        }
        drop(pipeline);
        *lock(&self.state) = RecState::Finalizing;
        self.finalized(Some(message.to_string()));
    }

    fn finalized(&self, error: Option<String>) {
        self.reset_live();
        let _ = self.apply(Command::Finalized);
        *lock(&self.file) = None;
        if let Some(message) = error {
            self.host.error(&message);
        }
        self.host.library_changed();
        self.emit(None, None);
    }
}

#[cfg(test)]
mod desktop_tests {
    //! Need a real desktop session: `cargo test -p hp-screen --lib -- --ignored --test-threads=1`
    use super::*;
    use std::time::Instant;

    fn wait_for_idle(recorder: &Recorder) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while recorder.status().state != RecState::Idle {
            assert!(Instant::now() < deadline, "stuck in {:?}", recorder.status().state);
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// With the countdown off, a stop can arrive while the recording is still starting up.
    /// The session must end completely: nothing may keep capturing behind an idle UI.
    #[test]
    #[ignore = "needs a desktop session"]
    fn a_stop_during_startup_leaves_nothing_running() {
        let host = Arc::new(FakeHost::default());
        let recorder = Recorder::with_host(host.clone());
        let dir = std::env::temp_dir().join("hp-screen-tests").join(format!("recorder-{}", std::process::id()));
        let settings = Settings {
            output_dir: Some(dir),
            countdown_secs: 0,
            system_audio: false,
            mic: false,
            ..Settings::default()
        };
        for round in 0..6 {
            recorder.start(Target::Monitor { index: 0 }, settings.clone()).unwrap();
            // Stop as early as the state machine allows it.
            let deadline = Instant::now() + Duration::from_secs(5);
            while recorder.stop().is_err() {
                assert!(Instant::now() < deadline, "round {round}: never became stoppable");
                std::thread::yield_now();
            }
            wait_for_idle(&recorder);
            std::thread::sleep(Duration::from_millis(600)); // a late begin() would have stored its pipeline by now
            assert!(!recorder.has_pipeline(), "round {round}: a recording is still running while the state is Idle");
        }
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;
    use hp_core::stroke::{COLORS, Stroke, Tool};

    #[test]
    fn cancelling_clears_the_live_layer() {
        let recorder = Recorder::with_host(Arc::new(FakeHost::default()));
        let live = recorder.live();
        live.upsert_stroke(Stroke { id: 1, tool: Tool::Pen, color: COLORS[0], width: 4.0, points: vec![(1.0, 1.0)], finished_ms: None });
        live.set_drawing(true);
        live.set_camera(Some((Arc::new(Mutex::new(None)), hp_core::webcam::WebcamSettings::default())));
        let dir = std::env::temp_dir().join("hp-screen-tests").join("live-cancel");
        let settings = Settings { output_dir: Some(dir), countdown_secs: 3, ..Settings::default() };
        recorder.start(Target::Monitor { index: 0 }, settings).unwrap();
        recorder.cancel().unwrap();
        assert!(!live.is_drawing());
        assert_eq!(live.stroke_count(), 0);
        assert!(!live.has_camera(), "a webcam never carries over to the next recording");
    }
}

/// What the recorder needs from the application around it. Keeping this behind a trait lets
/// the session logic be exercised without a running Tauri app.
pub trait Host: Send + Sync {
    fn state(&self, status: &RecStatus);
    fn error(&self, message: &str);
    fn library_changed(&self);
    fn output_dir(&self, settings: &Settings) -> Result<PathBuf, String>;
}

struct AppHost(AppHandle);

impl Host for AppHost {
    fn state(&self, status: &RecStatus) {
        events::rec_state(&self.0, status);
    }
    fn error(&self, message: &str) {
        events::rec_error(&self.0, message);
    }
    fn library_changed(&self) {
        events::library_changed(&self.0);
    }
    fn output_dir(&self, settings: &Settings) -> Result<PathBuf, String> {
        output_dir(&self.0, settings)
    }
}

/// Test host: remembers errors and saves into the folder named in the settings.
#[cfg(test)]
#[derive(Default)]
struct FakeHost {
    errors: Mutex<Vec<String>>,
}

#[cfg(test)]
impl Host for FakeHost {
    fn state(&self, _status: &RecStatus) {}
    fn error(&self, message: &str) {
        lock(&self.errors).push(message.to_string());
    }
    fn library_changed(&self) {}
    fn output_dir(&self, settings: &Settings) -> Result<PathBuf, String> {
        let dir = settings.output_dir.clone().ok_or("no folder")?;
        std::fs::create_dir_all(dir.join(hp_core::library::DATA_DIR)).map_err(|e| e.to_string())?;
        Ok(dir)
    }
}
