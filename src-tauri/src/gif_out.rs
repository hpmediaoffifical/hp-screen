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
    /// The previous frame (RGBA): later frames store only what changed.
    previous: Option<Vec<u8>>,
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
        Ok(GifWriter { enc, width, height, delay_cs: 100.0 / fps.max(1) as f64, elapsed_cs: 0.0, written_cs: 0, previous: None })
    }

    /// Adds one top-down BGRA frame of the writer's size (`width * 4` bytes per row).
    pub fn write(&mut self, bgra: &[u8]) -> Result<(), String> {
        let len = (self.width * self.height * 4) as usize;
        if bgra.len() < len {
            return Err("Khung GIF sai kích thước".to_string());
        }
        let rgba: Vec<u8> = bgra[..len].chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], 255]).collect();
        let (w, h) = (self.width as usize, self.height as usize);
        // Store only the rectangle that changed; the rest of the picture stays from before.
        let changed = match &self.previous {
            None => Some((0, 0, w, h)),
            Some(prev) => changed_rect(prev, &rgba, w, h),
        };
        let mut frame = match changed {
            Some((x0, y0, cw, ch)) => {
                let mut part = Vec::with_capacity(cw * ch * 4);
                for y in y0..y0 + ch {
                    part.extend_from_slice(&rgba[(y * w + x0) * 4..(y * w + x0 + cw) * 4]);
                }
                let mut f = gif::Frame::from_rgba_speed(cw as u16, ch as u16, &mut part, QUANT_SPEED);
                f.left = x0 as u16;
                f.top = y0 as u16;
                f
            }
            // Nothing changed: one transparent pixel just to hold the time.
            None => gif::Frame::from_rgba_speed(1, 1, &mut [0, 0, 0, 0], QUANT_SPEED),
        };
        frame.dispose = gif::DisposalMethod::Keep;
        self.previous = Some(rgba);
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

/// The smallest rectangle (x, y, w, h) holding every pixel that differs, or `None`.
fn changed_rect(a: &[u8], b: &[u8], w: usize, h: usize) -> Option<(usize, usize, usize, usize)> {
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        let row = y * w * 4;
        for x in 0..w {
            let i = row + x * 4;
            if a[i..i + 3] != b[i..i + 3] {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    (x0 < x1).then(|| (x0, y0, x1 - x0, y1 - y0))
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
    fn unchanged_frames_cost_almost_nothing() {
        let one = temp_path("gif-one", "a.gif");
        let many = temp_path("gif-many", "a.gif");
        let busy: Vec<u8> = (0..120 * 90).flat_map(|i| [(i * 7 % 256) as u8, (i * 13 % 256) as u8, (i * 29 % 256) as u8, 255]).collect();
        let mut g = GifWriter::new(&one, 120, 90, 15).unwrap();
        g.write(&busy).unwrap();
        g.finish().unwrap();
        let mut g = GifWriter::new(&many, 120, 90, 15).unwrap();
        for _ in 0..30 {
            g.write(&busy).unwrap();
        }
        g.finish().unwrap();
        let (a, b) = (std::fs::metadata(&one).unwrap().len(), std::fs::metadata(&many).unwrap().len());
        assert!(b < a * 2, "30 identical frames ({b} B) cost little more than one ({a} B)");
        assert_eq!(read_back(&many).2.len(), 30, "still 30 frames for the timing");
    }

    #[test]
    fn only_the_changed_area_is_stored() {
        let path = temp_path("gif-diff", "a.gif");
        let mut g = GifWriter::new(&path, 40, 30, 10).unwrap();
        let red: Vec<u8> = [0u8, 0, 255, 255].repeat(40 * 30);
        let mut changed = red.clone();
        for y in 10..15 {
            for x in 20..30 {
                let i = (y * 40 + x) * 4;
                changed[i..i + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
        g.write(&red).unwrap();
        g.write(&changed).unwrap();
        g.finish().unwrap();
        let mut dec = gif::DecodeOptions::new().read_info(std::fs::File::open(&path).unwrap()).unwrap();
        dec.read_next_frame().unwrap();
        let f = dec.read_next_frame().unwrap().unwrap();
        assert_eq!((f.left, f.top, f.width, f.height), (20, 10, 10, 5));
    }

    #[test]
    fn a_frame_of_the_wrong_size_is_refused() {
        let path = temp_path("gif-size", "a.gif");
        let mut g = GifWriter::new(&path, 40, 30, 10).unwrap();
        assert!(g.write(&[0u8; 16]).is_err());
    }
}
