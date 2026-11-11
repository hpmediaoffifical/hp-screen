//! Animated GIF output: each frame gets its own 256-colour palette (NeuQuant in the `gif`
//! crate), looping forever.
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// NeuQuant sampling: 1 is best and slowest, 30 fastest; 10 is the crate's balance.
const QUANT_SPEED: i32 = 10;

pub struct GifWriter {
    enc: gif::Encoder<BufWriter<File>>,
    width: u32,
    height: u32,
    /// Hundredths of a second per frame, kept fractional so the total time stays right.
    delay_cs: f64,
    elapsed_cs: f64,
    written_cs: u32,
}

fn err(context: &str) -> impl Fn(gif::EncodingError) -> String + '_ {
    move |e| format!("{context}: {e}")
}

impl GifWriter {
    pub fn new(path: &Path, width: u32, height: u32, fps: u32) -> Result<GifWriter, String> {
        if width == 0 || height == 0 || width > u16::MAX as u32 || height > u16::MAX as u32 {
            return Err(format!("Kích thước GIF không hợp lệ: {width}x{height}"));
        }
        let file = File::create(path).map_err(|e| format!("Không tạo được file GIF: {e}"))?;
        let mut enc = gif::Encoder::new(BufWriter::new(file), width as u16, height as u16, &[]).map_err(err("Không tạo được file GIF"))?;
        enc.set_repeat(gif::Repeat::Infinite).map_err(err("Không tạo được file GIF"))?;
        Ok(GifWriter { enc, width, height, delay_cs: 100.0 / fps.max(1) as f64, elapsed_cs: 0.0, written_cs: 0 })
    }

    /// Adds one top-down BGRA frame of the writer's size (`width * 4` bytes per row).
    pub fn write(&mut self, bgra: &[u8]) -> Result<(), String> {
        let len = (self.width * self.height * 4) as usize;
        if bgra.len() < len {
            return Err("Khung GIF sai kích thước".to_string());
        }
        let mut rgba: Vec<u8> = bgra[..len].chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], 255]).collect();
        let mut frame = gif::Frame::from_rgba_speed(self.width as u16, self.height as u16, &mut rgba, QUANT_SPEED);
        // GIF delays are whole hundredths: carry the remainder to the next frame.
        self.elapsed_cs += self.delay_cs;
        let delay = (self.elapsed_cs.round() as u32).saturating_sub(self.written_cs).max(1);
        self.written_cs += delay;
        frame.delay = delay.min(u16::MAX as u32) as u16;
        self.enc.write_frame(&frame).map_err(err("Ghi GIF thất bại"))
    }

    pub fn finish(self) -> Result<(), String> {
        let mut w = self.enc.into_inner().map_err(|e| format!("Không đóng được file GIF: {e}"))?;
        w.flush().map_err(|e| format!("Không đóng được file GIF: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_media::temp_path;

    fn read_back(path: &Path) -> (u16, u16, Vec<u16>, [u8; 3]) {
        let mut opts = gif::DecodeOptions::new();
        opts.set_color_output(gif::ColorOutput::RGBA);
        let mut dec = opts.read_info(std::fs::File::open(path).unwrap()).unwrap();
        let (w, h) = (dec.width(), dec.height());
        let mut delays = Vec::new();
        let mut first = None;
        while let Some(f) = dec.read_next_frame().unwrap() {
            delays.push(f.delay);
            if first.is_none() {
                let i = ((h as usize / 2) * w as usize + w as usize / 2) * 4;
                first = Some([f.buffer[i], f.buffer[i + 1], f.buffer[i + 2]]);
            }
        }
        (w, h, delays, first.unwrap_or_default())
    }

    #[test]
    fn frames_size_timing_and_colour_survive() {
        let path = temp_path("gif-basic", "a.gif");
        let mut g = GifWriter::new(&path, 40, 30, 15).unwrap();
        let red: Vec<u8> = [0u8, 0, 255, 255].repeat(40 * 30);
        for _ in 0..15 {
            g.write(&red).unwrap();
        }
        g.finish().unwrap();
        let (w, h, delays, centre) = read_back(&path);
        assert_eq!((w, h), (40, 30));
        assert_eq!(delays.len(), 15);
        let total: u32 = delays.iter().map(|d| *d as u32).sum();
        assert!((99..=101).contains(&total), "15 frames at 15 fps last one second: {total} cs");
        assert!(centre[0] > 240 && centre[1] < 20 && centre[2] < 20, "{centre:?}");
    }

    #[test]
    fn a_frame_of_the_wrong_size_is_refused() {
        let path = temp_path("gif-size", "a.gif");
        let mut g = GifWriter::new(&path, 40, 30, 10).unwrap();
        assert!(g.write(&[0u8; 16]).is_err());
    }
}
