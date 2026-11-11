//! Sample videos for tests, written with the app's own encoder (no desktop needed).
use crate::encoder::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, EncoderConfig, Mp4Encoder};
use std::path::{Path, PathBuf};

pub const FPS: u32 = 30;

/// Colour of the frame at a given second: red, green, blue, then red again.
pub fn colour_for_second(s: u32) -> [u8; 3] {
    match s % 3 {
        0 => [255, 0, 0],
        1 => [0, 255, 0],
        _ => [0, 0, 255],
    }
}

/// A fresh path in a per-test temp folder (`name` may contain any characters).
pub fn temp_path(folder: &str, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("hp-screen-tests").join(folder);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// Writes `secs` seconds of `w`×`h` video at 30 fps. Each frame is one solid colour
/// (`colour_for_second`); with `audio`, a 440 Hz tone.
pub fn make_sample(path: &Path, secs: u32, w: u32, h: u32, audio: bool) {
    let cfg = EncoderConfig { path: path.to_path_buf(), width: w, height: h, fps: FPS, video_bitrate: 2_000_000, audio };
    let mut enc = Mp4Encoder::new(&cfg).expect("encoder opens");
    let mut frame = vec![0u8; (w * h * 4) as usize];
    for n in 0..secs * FPS {
        let [r, g, b] = colour_for_second(n / FPS);
        for px in frame.chunks_exact_mut(4) {
            px.copy_from_slice(&[b, g, r, 255]);
        }
        enc.write_video(&frame, w * 4, n as i64 * 10_000_000 / FPS as i64).unwrap();
    }
    if audio {
        let chunk = AUDIO_SAMPLE_RATE / 10;
        for c in 0..secs * 10 {
            let pcm: Vec<f32> = (0..chunk)
                .flat_map(|i| {
                    let t = (c * chunk + i) as f32 / AUDIO_SAMPLE_RATE as f32;
                    let v = (t * 440.0 * std::f32::consts::TAU).sin() * 0.3;
                    std::iter::repeat_n(v, AUDIO_CHANNELS as usize)
                })
                .collect();
            enc.write_audio(&pcm, c as i64 * 1_000_000).unwrap();
        }
    }
    enc.finish().unwrap();
}

/// Centre pixel of a BGRA frame as RGB.
pub fn centre_rgb(bgra: &[u8], w: u32, h: u32) -> [u8; 3] {
    let i = ((h / 2 * w + w / 2) * 4) as usize;
    [bgra[i + 2], bgra[i + 1], bgra[i]]
}

/// True when `rgb` is close to `want` (H.264 is lossy).
pub fn near(rgb: [u8; 3], want: [u8; 3]) -> bool {
    rgb.iter().zip(want).all(|(a, b)| (*a as i32 - b as i32).abs() < 70)
}
