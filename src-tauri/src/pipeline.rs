//! One recording from capture to file, independent of the UI: capture frames, pace them to a
//! constant frame rate on the shared timeline, encode, and report progress through a callback.
use crate::audio::{self, AudioConfig, AudioSession};
use crate::capture::{self, CaptureSession, Frame, FrameSink, Target};
use crate::clock;
use crate::encoder::{self, EncoderConfig, Mp4Encoder};
use hp_core::quality::{Quality, video_bitrate};
use hp_core::timeline::Timeline;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::IsIconic;

/// Longest run of repeated frames written in one tick when the encoder has fallen behind.
const MAX_CATCH_UP_FRAMES: u32 = 4;
const TICK_EVENT_INTERVAL: Duration = Duration::from_millis(500);
const THUMB_WIDTH: u32 = 320;

#[derive(Clone, Debug)]
pub struct PipelineConfig {
    pub target: Target,
    pub path: PathBuf,
    /// Where to save a small preview image of the first frame.
    pub thumb: Option<PathBuf>,
    pub fps: u32,
    pub quality: Quality,
    /// Which audio sources to record; `None` writes a video without an audio track.
    pub audio: Option<AudioConfig>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PipelineEvent {
    /// Sent about twice a second while the pipeline runs.
    Tick { elapsed_ms: i64, warning: Option<String> },
    /// The captured window was closed.
    SourceClosed,
    /// Recording cannot continue; the file was closed as far as possible.
    Failed(String),
}

pub type EventFn = Arc<dyn Fn(PipelineEvent) + Send + Sync>;

/// The encoder, shared so the audio thread can write to the same file.
pub type SharedEncoder = Arc<Mutex<Option<Mp4Encoder>>>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    pub frames: u64,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
}

pub struct Pipeline {
    stop: Arc<AtomicBool>,
    timeline: Arc<Mutex<Timeline>>,
    encoder: SharedEncoder,
    capture: Option<CaptureSession>,
    audio: Option<AudioSession>,
    pacer: Option<JoinHandle<Result<Stats, String>>>,
}

/// Most recent captured frame, copied out of the capture callback.
#[derive(Default)]
struct Latest {
    data: Vec<u8>,
    width: u32,
    height: u32,
    stride: u32,
    present: bool,
}

struct LatestSink {
    latest: Arc<Mutex<Latest>>,
    events: EventFn,
}

impl FrameSink for LatestSink {
    fn on_frame(&mut self, f: Frame<'_>) {
        let mut latest = lock(&self.latest);
        latest.data.clear();
        latest.data.extend_from_slice(f.bgra);
        latest.width = f.width;
        latest.height = f.height;
        latest.stride = f.stride;
        latest.present = true;
    }

    fn on_closed(&mut self) {
        (self.events)(PipelineEvent::SourceClosed);
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Pipeline {
    pub fn start(cfg: PipelineConfig, events: EventFn) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let timeline = Arc::new(Mutex::new(Timeline::new(clock::now_100ns(), clock::TICKS_PER_SEC)));
        let encoder: SharedEncoder = Arc::new(Mutex::new(None));
        let latest = Arc::new(Mutex::new(Latest::default()));

        let sink = LatestSink { latest: latest.clone(), events: events.clone() };
        let capture = capture::start_capture(&cfg.target, Box::new(sink))?;

        let audio_cfg = cfg.audio;
        let pacer = {
            let (stop, timeline, encoder) = (stop.clone(), timeline.clone(), encoder.clone());
            std::thread::Builder::new()
                .name("hp-pacer".into())
                .spawn(move || pace(cfg, stop, timeline, encoder, latest, events))
                .map_err(|e| format!("Không tạo được luồng ghi hình: {e}"))?
        };
        let mut pipeline = Self { stop, timeline, encoder, capture: Some(capture), audio: None, pacer: Some(pacer) };
        if let Some(audio_cfg) = audio_cfg {
            let encoder = pipeline.encoder.clone();
            // Audio that arrives before the first video frame has no file to go to yet and is
            // dropped; a failed audio write must not end the recording.
            let sink = Box::new(move |pcm: &[f32], time_100ns: i64| {
                if let Some(enc) = lock(&encoder).as_mut() {
                    let _ = enc.write_audio(pcm, time_100ns);
                }
            });
            // On error `pipeline` is dropped here, which stops the capture and the pacer.
            pipeline.audio = Some(audio::start_audio(audio_cfg, pipeline.timeline.clone(), sink)?);
        }
        Ok(pipeline)
    }

    pub fn timeline(&self) -> Arc<Mutex<Timeline>> {
        self.timeline.clone()
    }

    pub fn encoder(&self) -> SharedEncoder {
        self.encoder.clone()
    }

    pub fn pause(&self) {
        lock(&self.timeline).pause(clock::now_100ns());
    }

    pub fn resume(&self) {
        lock(&self.timeline).resume(clock::now_100ns());
    }

    /// Stops capturing, flushes the encoder and closes the file.
    pub fn finish(mut self) -> Result<Stats, String> {
        self.shutdown()
    }

    fn shutdown(&mut self) -> Result<Stats, String> {
        self.stop.store(true, Ordering::SeqCst);
        self.audio.take();
        self.capture.take();
        match self.pacer.take() {
            Some(handle) => handle.join().unwrap_or_else(|_| Err("Luồng ghi hình dừng bất thường".to_string())),
            None => Err("Phiên quay đã kết thúc".to_string()),
        }
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        if self.pacer.is_some() {
            let _ = self.shutdown();
        }
    }
}

fn pace(
    cfg: PipelineConfig,
    stop: Arc<AtomicBool>,
    timeline: Arc<Mutex<Timeline>>,
    encoder: SharedEncoder,
    latest: Arc<Mutex<Latest>>,
    events: EventFn,
) -> Result<Stats, String> {
    encoder::init_thread();
    let result = pace_loop(&cfg, &stop, &timeline, &encoder, &latest, &events);
    // Close the file whatever happened, so what was recorded stays playable.
    let finished = match lock(&encoder).take() {
        Some(enc) => enc.finish(),
        None => Ok(()),
    };
    match (result, finished) {
        (Ok(stats), Ok(())) if stats.frames > 0 => Ok(stats),
        (Ok(_), Ok(())) => Err("Không ghi được khung hình nào".to_string()),
        (Ok(_), Err(e)) => Err(e),
        (Err(e), _) => {
            events(PipelineEvent::Failed(e.clone()));
            Err(e)
        }
    }
}

fn pace_loop(
    cfg: &PipelineConfig,
    stop: &AtomicBool,
    timeline: &Mutex<Timeline>,
    encoder: &SharedEncoder,
    latest: &Mutex<Latest>,
    events: &EventFn,
) -> Result<Stats, String> {
    let fps = cfg.fps.max(1);
    let frame_duration = clock::TICKS_PER_SEC / fps as i64;
    let tick = Duration::from_nanos(1_000_000_000 / fps as u64);
    let mut canvas: Vec<u8> = Vec::new();
    let (mut width, mut height) = (0u32, 0u32);
    let mut written: i64 = 0;
    let mut elapsed_ms: i64 = 0;
    let mut next_tick = Instant::now();
    let mut last_event = Instant::now() - TICK_EVENT_INTERVAL;

    while !stop.load(Ordering::SeqCst) {
        let video_time = lock(timeline).video_100ns(clock::now_100ns());
        if let Some(video_time) = video_time {
            elapsed_ms = video_time / 10_000;
            {
                let frame = lock(latest);
                if frame.present {
                    if width == 0 {
                        // The first frame fixes the video size for the whole recording.
                        (width, height) = (frame.width & !1, frame.height & !1);
                        canvas = vec![0; width as usize * height as usize * 4];
                        let enc = Mp4Encoder::new(&EncoderConfig {
                            path: cfg.path.clone(),
                            width,
                            height,
                            fps,
                            video_bitrate: video_bitrate(width, height, fps, cfg.quality),
                            audio: cfg.audio.is_some(),
                        })?;
                        *lock(encoder) = Some(enc);
                    }
                    blit(&frame, &mut canvas, width, height);
                }
            }
            if width != 0 {
                if written == 0
                    && let Some(thumb) = &cfg.thumb {
                        save_thumbnail(&canvas, width, height, thumb);
                    }
                // Keep the frame count in step with the clock: repeat the current picture when
                // nothing changed on screen, skip ahead when the encoder cannot keep up.
                let due = video_time / frame_duration;
                let mut guard = lock(encoder);
                let enc = guard.as_mut().ok_or("Bộ mã hóa đã đóng")?;
                let mut burst = 0;
                while written <= due && burst < MAX_CATCH_UP_FRAMES {
                    enc.write_video(&canvas, width * 4, written * frame_duration)?;
                    written += 1;
                    burst += 1;
                }
                if written <= due {
                    written = due + 1;
                }
            }
        }

        if last_event.elapsed() >= TICK_EVENT_INTERVAL {
            last_event = Instant::now();
            events(PipelineEvent::Tick { elapsed_ms, warning: source_warning(&cfg.target) });
        }

        next_tick += tick;
        match next_tick.checked_duration_since(Instant::now()) {
            Some(wait) => std::thread::sleep(wait),
            None => next_tick = Instant::now(), // running late; do not try to make up sleeps
        }
    }
    Ok(Stats { frames: written.max(0) as u64, width, height, fps })
}

/// Copies the frame into the fixed-size canvas. A window that changed size is cropped or
/// padded with black, because the video size cannot change mid-file.
fn blit(frame: &Latest, canvas: &mut [u8], width: u32, height: u32) {
    let copy_w = frame.width.min(width) as usize * 4;
    let copy_h = frame.height.min(height) as usize;
    let canvas_row = width as usize * 4;
    if frame.width != width || frame.height != height {
        canvas.fill(0);
    }
    for (dst, src) in canvas.chunks_exact_mut(canvas_row).zip(frame.data.chunks(frame.stride.max(1) as usize)).take(copy_h) {
        if src.len() >= copy_w {
            dst[..copy_w].copy_from_slice(&src[..copy_w]);
        }
    }
}

fn source_warning(target: &Target) -> Option<String> {
    let Target::Window { hwnd } = target else { return None };
    // SAFETY: IsIconic accepts any handle value and only reads window state.
    let minimized = unsafe { IsIconic(HWND(*hwnd as *mut _)) }.as_bool();
    minimized.then(|| "Cửa sổ đang bị thu nhỏ: video giữ khung hình cuối cho đến khi mở lại.".to_string())
}

/// Best effort: a missing thumbnail only means the library shows a placeholder.
fn save_thumbnail(bgra: &[u8], width: u32, height: u32, path: &std::path::Path) {
    let step = width.div_ceil(THUMB_WIDTH).max(1) as usize;
    let (tw, th) = (width as usize / step, height as usize / step);
    if tw == 0 || th == 0 {
        return;
    }
    let mut rgba = Vec::with_capacity(tw * th * 4);
    for y in 0..th {
        for x in 0..tw {
            let i = (y * step * width as usize + x * step) * 4;
            rgba.extend_from_slice(&[bgra[i + 2], bgra[i + 1], bgra[i], 255]);
        }
    }
    if let Some(img) = image::RgbaImage::from_raw(tw as u32, th as u32, rgba) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = img.save(path);
    }
}

#[cfg(test)]
mod desktop_tests {
    //! Need a real desktop session: `cargo test -p hp-screen --lib -- --ignored --nocapture`
    use super::*;
    use crate::capture::list_monitors;

    fn temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("hp-screen-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let _ = std::fs::remove_file(&path);
        path
    }

    fn record(target: Target, name: &str, script: impl FnOnce(&Pipeline)) -> (Stats, (u32, i64, u32), Vec<PipelineEvent>) {
        record_with(target, name, None, script)
    }

    fn record_with(
        target: Target,
        name: &str,
        audio: Option<AudioConfig>,
        script: impl FnOnce(&Pipeline),
    ) -> (Stats, (u32, i64, u32), Vec<PipelineEvent>) {
        let path = temp_file(name);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let events: EventFn = {
            let seen = seen.clone();
            Arc::new(move |e| seen.lock().unwrap().push(e))
        };
        let cfg = PipelineConfig { target, path: path.clone(), thumb: None, fps: 30, quality: Quality::Standard, audio };
        let pipeline = Pipeline::start(cfg, events).expect("pipeline starts");
        script(&pipeline);
        let stats = pipeline.finish().expect("pipeline finishes");
        let probed = encoder::probe(&path).expect("file can be read back");
        let events = seen.lock().unwrap().clone();
        (stats, probed, events)
    }

    #[test]
    #[ignore = "needs a desktop session"]
    fn three_seconds_of_the_primary_monitor_give_about_ninety_frames() {
        let primary = list_monitors().into_iter().find(|m| m.primary).unwrap();
        let (stats, (frames, last, _), events) = record(Target::Monitor { index: primary.index }, "monitor.mp4", |_| {
            std::thread::sleep(Duration::from_secs(3));
        });
        println!("stats {stats:?}, probed frames {frames}, last {:.2}s, {} events", last as f64 / 1e7, events.len());
        // The test process is not DPI aware, so compare with a screenshot rather than the
        // (virtualized) monitor rectangle.
        let shot = capture::grab_frame(&Target::Monitor { index: primary.index }).unwrap();
        assert_eq!((stats.width, stats.height), (shot.width() & !1, shot.height() & !1));
        assert!((80..=100).contains(&frames), "expected about 90 frames, got {frames}");
        assert!((2.6e7..=3.4e7).contains(&(last as f64)), "last frame at {last}");
        assert!(events.iter().any(|e| matches!(e, PipelineEvent::Tick { .. })));
        assert!(!events.iter().any(|e| matches!(e, PipelineEvent::Failed(_))));
    }

    #[test]
    #[ignore = "needs a desktop session"]
    fn recording_with_audio_writes_an_audio_track_as_long_as_the_video() {
        let primary = list_monitors().into_iter().find(|m| m.primary).unwrap();
        let rect = hp_core::geometry::Rect { x: primary.rect.x + 100, y: primary.rect.y + 100, w: 640, h: 480 };
        let audio = Some(AudioConfig { system: true, mic: true });
        println!("audio status: {:?}", audio::audio_status());
        let (_, (frames, last, audio_samples), events) =
            record_with(Target::Region { monitor: primary.index, rect }, "audio.mp4", audio, |_| {
                std::thread::sleep(Duration::from_secs(3));
            });
        println!("video frames {frames}, last {:.2}s, audio samples {audio_samples}", last as f64 / 1e7);
        assert!((80..=100).contains(&frames));
        // AAC packs 1024 frames per sample: 3 s at 48 kHz is about 140 samples.
        assert!((110..=170).contains(&audio_samples), "audio samples {audio_samples}");
        assert!(!events.iter().any(|e| matches!(e, PipelineEvent::Failed(_))));
    }

    #[test]
    #[ignore = "needs a desktop session"]
    fn paused_time_is_left_out_of_the_video() {
        let primary = list_monitors().into_iter().find(|m| m.primary).unwrap();
        let rect = hp_core::geometry::Rect { x: primary.rect.x + 100, y: primary.rect.y + 100, w: 641, h: 481 };
        let (stats, (frames, last, _), _) = record(Target::Region { monitor: primary.index, rect }, "paused.mp4", |p| {
            std::thread::sleep(Duration::from_millis(1500));
            p.pause();
            std::thread::sleep(Duration::from_millis(2000));
            p.resume();
            std::thread::sleep(Duration::from_millis(1500));
        });
        println!("stats {stats:?}, probed frames {frames}, last {:.2}s", last as f64 / 1e7);
        assert_eq!((stats.width, stats.height), (640, 480));
        assert!((80..=100).contains(&frames), "3 s recorded + 2 s paused should give about 90 frames, got {frames}");
        assert!((2.6e7..=3.4e7).contains(&(last as f64)), "last frame at {last}");
    }
}

#[cfg(test)]
mod size_tests {
    use super::*;
    use crate::capture::list_monitors;

    /// Sizes that are even but not multiples of 16 must still record (found with a 698x450 region).
    #[test]
    #[ignore = "needs a desktop session"]
    fn awkward_even_sizes_record_without_error() {
        let primary = list_monitors().into_iter().find(|m| m.primary).unwrap();
        let sizes: Vec<(u32, u32)> = match std::env::var("HP_SIZES") {
            Ok(list) => list.split(',').map(|s| { let (w, h) = s.split_once('x').unwrap(); (w.parse().unwrap(), h.parse().unwrap()) }).collect(),
            Err(_) => vec![(698, 450), (1002, 562)],
        };
        for (w, h) in sizes {
            let path = std::env::temp_dir().join("hp-screen-tests").join(format!("size-{w}x{h}.mp4"));
            let _ = std::fs::remove_file(&path);
            let rect = hp_core::geometry::Rect { x: primary.rect.x + 50, y: primary.rect.y + 50, w, h };
            let seen = Arc::new(Mutex::new(Vec::new()));
            let events: EventFn = {
                let seen = seen.clone();
                Arc::new(move |e| seen.lock().unwrap().push(e))
            };
            let cfg = PipelineConfig {
                target: Target::Region { monitor: primary.index, rect },
                path: path.clone(),
                thumb: None,
                fps: 30,
                quality: Quality::Standard,
                audio: Some(AudioConfig { system: true, mic: true }),
            };
            let pipeline = Pipeline::start(cfg, events).expect("pipeline starts");
            std::thread::sleep(Duration::from_secs(2));
            let result = pipeline.finish();
            let failures: Vec<_> = seen.lock().unwrap().iter().filter(|e| matches!(e, PipelineEvent::Failed(_))).cloned().collect();
            println!("{w}x{h}: {result:?} failures {failures:?} bytes {:?}", std::fs::metadata(&path).map(|m| m.len()));
            if std::env::var("HP_SIZES").is_ok() { continue; }
            assert!(result.is_ok(), "{w}x{h} failed: {result:?}");
            let (frames, _, _) = encoder::probe(&path).expect("readable");
            assert!(frames >= 45, "{w}x{h}: only {frames} frames");
        }
    }
}
