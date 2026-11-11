//! The recording session as the UI sees it: the state machine from `hp_core::state` driving a
//! `Pipeline`, with every change reported to the UI as a `rec-state` event.
use crate::capture::Target;
use crate::events;
use crate::paths::{new_media_path, output_dir};
use crate::pipeline::{EventFn, Pipeline, PipelineConfig, PipelineEvent};
use hp_core::library::thumb_path;
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
    app: AppHandle,
    state: Mutex<RecState>,
    pipeline: Mutex<Option<Pipeline>>,
    file: Mutex<Option<PathBuf>>,
    last_target: Mutex<Option<Target>>,
    elapsed_ms: AtomicI64,
    /// Bumped on every start so a cancelled countdown cannot start a later session.
    generation: AtomicU64,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Recorder {
    pub fn new(app: AppHandle) -> Self {
        Self {
            inner: Arc::new(Inner {
                app,
                state: Mutex::new(RecState::Idle),
                pipeline: Mutex::new(None),
                file: Mutex::new(None),
                last_target: Mutex::new(None),
                elapsed_ms: AtomicI64::new(0),
                generation: AtomicU64::new(0),
            }),
        }
    }

    pub fn status(&self) -> RecStatus {
        self.inner.status(None, None)
    }

    pub fn last_target(&self) -> Option<Target> {
        lock(&self.inner.last_target).clone()
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
        events::rec_state(&self.app, &self.status(countdown, warning));
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
        if !still_ours(&self) || self.apply(Command::CountdownDone).is_err() {
            return;
        }
        if let Err(message) = self.clone().begin(target, &settings) {
            *lock(&self.state) = RecState::Idle;
            *lock(&self.file) = None;
            events::rec_error(&self.app, &message);
        }
        self.emit(None, None);
    }

    fn begin(self: Arc<Self>, target: Target, settings: &Settings) -> Result<(), String> {
        let dir = output_dir(&self.app, settings)?;
        let path = new_media_path(&dir, settings, "mp4");
        let cfg = PipelineConfig {
            target,
            path: path.clone(),
            thumb: Some(thumb_path(&path)),
            fps: settings.fps,
            quality: settings.quality,
            audio: (settings.system_audio || settings.mic)
                .then_some(crate::audio::AudioConfig { system: settings.system_audio, mic: settings.mic }),
        };
        let weak = Arc::downgrade(&self);
        let on_event: EventFn = Arc::new(move |event| {
            let Some(inner) = weak.upgrade() else { return };
            match event {
                PipelineEvent::Tick { elapsed_ms, warning } => {
                    inner.elapsed_ms.store(elapsed_ms, Ordering::SeqCst);
                    inner.emit(None, warning);
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
        let pipeline = Pipeline::start(cfg, on_event)?;
        *lock(&self.file) = Some(path);
        *lock(&self.pipeline) = Some(pipeline);
        Ok(())
    }

    fn stop(self: Arc<Self>) -> Result<(), String> {
        self.apply(Command::Stop)?;
        self.emit(None, None);
        std::thread::spawn(move || {
            let pipeline = lock(&self.pipeline).take();
            let result = match pipeline {
                Some(pipeline) => pipeline.finish().map(|_| ()),
                None => Ok(()),
            };
            self.finalized(result.err());
        });
        Ok(())
    }

    /// The pipeline reported an unrecoverable error while recording.
    fn fail(self: Arc<Self>, message: &str) {
        let pipeline = lock(&self.pipeline).take();
        if pipeline.is_none() {
            return; // a stop is already closing the file and will report the outcome
        }
        drop(pipeline);
        *lock(&self.state) = RecState::Finalizing;
        self.finalized(Some(message.to_string()));
    }

    fn finalized(&self, error: Option<String>) {
        let _ = self.apply(Command::Finalized);
        *lock(&self.file) = None;
        if let Some(message) = error {
            events::rec_error(&self.app, &message);
        }
        events::library_changed(&self.app);
        self.emit(None, None);
    }
}
