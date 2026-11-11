//! System audio (WASAPI loopback) and microphone capture, mixed into one 48 kHz stereo stream.
//!
//! Both sources follow the Windows default devices: a supervisor notices when the default
//! changes or a stream breaks and rebuilds that source. The mixer is driven by the recording
//! timeline, so the audio it emits is continuous (silence fills gaps) and stays in step with
//! the video however the devices behave.
use crate::clock;
use crate::encoder::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Data, SampleFormat};
use hp_core::mix::{Resampler, mix_f32, to_stereo};
use hp_core::timeline::Timeline;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const RATE: i64 = AUDIO_SAMPLE_RATE as i64;
const CHANNELS: usize = AUDIO_CHANNELS as usize;
const MIX_INTERVAL: Duration = Duration::from_millis(10);
const SUPERVISE_INTERVAL: Duration = Duration::from_secs(1);
/// The mixer runs this far behind the clock so device callbacks have time to arrive.
const MIX_DELAY_100NS: i64 = 400_000; // 40 ms
/// Samples a source may hold before the oldest are dropped (one second).
const MAX_BUFFERED_SAMPLES: usize = AUDIO_SAMPLE_RATE as usize * CHANNELS;

#[derive(Clone, Copy, Debug)]
pub struct AudioConfig {
    pub system: bool,
    pub mic: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct AudioStatus {
    pub system_available: bool,
    pub mic_available: bool,
}

/// Receives mixed interleaved stereo samples and the video time of the first one (100 ns).
pub type AudioSink = Box<dyn FnMut(&[f32], i64) + Send>;

/// Whether Windows currently has a default playback device and a default microphone.
pub fn audio_status() -> AudioStatus {
    let host = cpal::default_host();
    AudioStatus {
        system_available: host.default_output_device().is_some(),
        mic_available: host.default_input_device().is_some(),
    }
}

/// Running audio capture. Dropping it stops the capture thread.
pub struct AudioSession {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for AudioSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn start_audio(cfg: AudioConfig, timeline: Arc<Mutex<Timeline>>, sink: AudioSink) -> Result<AudioSession, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let thread = {
        let stop = stop.clone();
        std::thread::Builder::new()
            .name("hp-audio".into())
            .spawn(move || run(cfg, &stop, &timeline, sink))
            .map_err(|e| format!("Không tạo được luồng thu âm: {e}"))?
    };
    Ok(AudioSession { stop, thread: Some(thread) })
}

fn run(cfg: AudioConfig, stop: &AtomicBool, timeline: &Mutex<Timeline>, mut sink: AudioSink) {
    let host = cpal::default_host();
    let mut sources: Vec<Source> = [(SourceKind::System, cfg.system), (SourceKind::Mic, cfg.mic)]
        .into_iter()
        .filter(|(_, enabled)| *enabled)
        .map(|(kind, _)| Source::new(kind))
        .collect();
    let mut emitted_frames: i64 = 0;
    let mut last_supervised: Option<Instant> = None;

    while !stop.load(Ordering::SeqCst) {
        if last_supervised.is_none_or(|t| t.elapsed() >= SUPERVISE_INTERVAL) {
            last_supervised = Some(Instant::now());
            for source in &mut sources {
                source.supervise(&host);
            }
        }

        let video_time = timeline.lock().unwrap_or_else(|e| e.into_inner()).video_100ns(clock::now_100ns());
        match video_time {
            // Paused: discard what arrives so it does not play back after resuming.
            None => sources.iter().for_each(Source::clear),
            Some(video_time) => {
                let target = (video_time - MIX_DELAY_100NS).max(0) * RATE / clock::TICKS_PER_SEC;
                let mut frames = target - emitted_frames;
                if frames > RATE {
                    // The thread was stalled for over a second; do not emit a burst of stale audio.
                    emitted_frames = target - RATE / 100;
                    frames = RATE / 100;
                }
                if frames > 0 {
                    let mut mixed = vec![0.0f32; frames as usize * CHANNELS];
                    for source in &sources {
                        mixed = mix_f32(&mixed, &source.take(frames as usize));
                    }
                    sink(&mixed, emitted_frames * clock::TICKS_PER_SEC / RATE);
                    emitted_frames += frames;
                }
            }
        }
        std::thread::sleep(MIX_INTERVAL);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SourceKind {
    System,
    Mic,
}

struct Source {
    kind: SourceKind,
    ring: Arc<Mutex<VecDeque<f32>>>,
    broken: Arc<AtomicBool>,
    stream: Option<cpal::Stream>,
    device_id: Option<String>,
}

impl Source {
    fn new(kind: SourceKind) -> Self {
        Self { kind, ring: Arc::default(), broken: Arc::default(), stream: None, device_id: None }
    }

    fn clear(&self) {
        self.ring.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// Exactly `frames` stereo frames: buffered audio first, silence for whatever is missing.
    fn take(&self, frames: usize) -> Vec<f32> {
        let mut ring = self.ring.lock().unwrap_or_else(|e| e.into_inner());
        let wanted = frames * CHANNELS;
        let available = wanted.min(ring.len());
        let mut out: Vec<f32> = ring.drain(..available).collect();
        out.resize(wanted, 0.0);
        out
    }

    /// (Re)builds the stream when there is none, it broke, or the default device changed.
    fn supervise(&mut self, host: &cpal::Host) {
        let device = match self.kind {
            SourceKind::System => host.default_output_device(),
            SourceKind::Mic => host.default_input_device(),
        };
        let id = device.as_ref().and_then(|d| d.id().ok()).map(|id| id.to_string());
        let healthy = self.stream.is_some() && !self.broken.load(Ordering::SeqCst);
        if healthy && id == self.device_id {
            return;
        }
        self.stream = None; // stop the old stream before opening the new device
        self.broken.store(false, Ordering::SeqCst);
        self.device_id = id;
        self.clear();
        self.stream = device.and_then(|d| self.build(&d).ok());
    }

    fn build(&self, device: &cpal::Device) -> Result<cpal::Stream, cpal::Error> {
        // Opening an output device for input is how WASAPI loopback is requested.
        let supported = match self.kind {
            SourceKind::System => device.default_output_config()?,
            SourceKind::Mic => device.default_input_config()?,
        };
        let channels = supported.channels() as usize;
        let format = supported.sample_format();
        let mut resampler = Resampler::new(supported.sample_rate(), AUDIO_SAMPLE_RATE);
        let ring = self.ring.clone();
        let broken = self.broken.clone();
        let stream = device.build_input_stream_raw(
            supported.config(),
            format,
            move |data: &Data, _| {
                let samples = resampler.process(&to_stereo(&to_f32(data), channels));
                let mut ring = ring.lock().unwrap_or_else(|e| e.into_inner());
                ring.extend(samples);
                let excess = ring.len().saturating_sub(MAX_BUFFERED_SAMPLES);
                ring.drain(..excess);
            },
            move |_| broken.store(true, Ordering::SeqCst),
            None,
        )?;
        stream.play()?;
        Ok(stream)
    }
}

/// Device samples as floats in [-1, 1]. Unknown formats give silence rather than noise.
fn to_f32(data: &Data) -> Vec<f32> {
    match data.sample_format() {
        SampleFormat::F32 => data.as_slice::<f32>().map(<[f32]>::to_vec),
        SampleFormat::I16 => data.as_slice::<i16>().map(|s| s.iter().map(|v| *v as f32 / 32_768.0).collect()),
        SampleFormat::U16 => data.as_slice::<u16>().map(|s| s.iter().map(|v| (*v as f32 - 32_768.0) / 32_768.0).collect()),
        SampleFormat::I32 => data.as_slice::<i32>().map(|s| s.iter().map(|v| *v as f32 / 2_147_483_648.0).collect()),
        _ => None,
    }
    .unwrap_or_default()
}
