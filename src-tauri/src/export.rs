//! Writes an edited copy of a recording: decode, compose, encode into a `.part` file that is
//! renamed into place only when everything succeeded. The source file is only ever read.
use crate::compositor::{Compositor, Device};
use crate::decoder::{AudioDecoder, VideoDecoder};
use crate::encoder::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, EncoderConfig, Mp4Encoder};
use crate::library_fs::{stem_in_use, write_meta};
use hp_core::cut::Cuts;
use hp_core::library::{DATA_DIR, MediaMeta, thumb_path};
use hp_core::naming::unique_path;
use hp_core::plan::Plan;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub const EDIT_SUFFIX: &str = "_edit";
/// Audio is written up to this far ahead of the video so the writer can interleave.
const AUDIO_LEAD_MS: f64 = 500.0;
const PROGRESS_EVERY: u64 = 10;

/// `<dir>/<stem>_edit.mp4`, or the next free `_edit_2`, `_edit_3`…
pub fn export_path(source: &Path) -> PathBuf {
    export_path_ext(source, "mp4")
}

/// As `export_path`, with another file type (`gif`).
pub fn export_path_ext(source: &Path, ext: &str) -> PathBuf {
    let dir = source.parent().unwrap_or_else(|| Path::new(""));
    let stem = source.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let taken = |p: &Path| p.exists() || p.file_stem().is_some_and(|s| stem_in_use(dir, &s.to_string_lossy()));
    unique_path(dir, &format!("{stem}{EDIT_SUFFIX}"), ext, &taken)
}

/// Moves the finished `part` to `out`, or to the next free name if `out` was taken meanwhile.
fn place(source: &Path, part: &Path, out: &Path) -> Result<PathBuf, String> {
    let ext = out.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_else(|| "mp4".into());
    let mut out = out.to_path_buf();
    let mut tries = 0;
    loop {
        match move_no_replace(part, &out) {
            Ok(()) => return Ok(out),
            Err(_) if out.exists() && tries < 50 => {
                tries += 1;
                out = export_path_ext(source, &ext);
            }
            Err(e) => {
                let _ = std::fs::remove_file(part);
                return Err(format!("Không lưu được file xuất: {e}"));
            }
        }
    }
}

/// Exports `plan` (made with `Plan::for_gif`) of `source` as an animated GIF; no sound.
pub fn run_export_gif(source: &Path, out: &Path, plan: &Plan, progress: &mut dyn FnMut(u64, u64), cancel: &AtomicBool) -> Result<PathBuf, String> {
    let part = part_path(out);
    if let Some(dir) = part.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Không tạo được thư mục tạm: {e}"))?;
    }
    let written = (|| -> Result<(), String> {
        let (mut video, info) = VideoDecoder::open(source)?;
        let (w, h) = plan.out_size();
        let mut gif = crate::gif_out::GifWriter::new(&part, w, h, plan.fps())?;
        let mut compositor = Compositor::new(w, h, Device::Hardware)?;
        let total = plan.frames();
        for n in 0..total {
            if cancel.load(Ordering::Relaxed) {
                return Err("Đã hủy".to_string());
            }
            let Some(fp) = plan.frame(n) else { break };
            let src = video.frame_at(fp.src_ms)?;
            let frame = compositor.render(src, info.width, info.height, Some(fp.view), Some(plan.layout()), &fp.draws)?;
            gif.write(frame)?;
            if (n + 1) % PROGRESS_EVERY == 0 || n + 1 == total {
                progress(n + 1, total);
            }
        }
        gif.finish()
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    place(source, &part, out)
}

/// The temporary file an export writes before it is renamed to `out`.
pub fn part_path(out: &Path) -> PathBuf {
    let dir = out.parent().unwrap_or_else(|| Path::new(""));
    let name = out.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    dir.join(DATA_DIR).join(format!("{name}.part"))
}

/// Moves `from` to `to`, failing (instead of replacing) when `to` exists.
fn move_no_replace(from: &Path, to: &Path) -> Result<(), String> {
    use windows::Win32::Storage::FileSystem::{MOVE_FILE_FLAGS, MoveFileExW};
    use windows::core::HSTRING;
    // SAFETY: two valid wide strings; no flags means "fail if the target exists".
    unsafe { MoveFileExW(&HSTRING::from(from.as_os_str()), &HSTRING::from(to.as_os_str()), MOVE_FILE_FLAGS(0)) }.map_err(|e| e.to_string())
}

/// Exports `plan` of `source` to `out`, or to the next free name if `out` was taken while
/// exporting; returns the file written. `progress(done, total)` is called every few frames.
pub fn run_export(
    source: &Path,
    out: &Path,
    plan: &Plan,
    bitrate: u32,
    progress: &mut dyn FnMut(u64, u64),
    cancel: &AtomicBool,
) -> Result<PathBuf, String> {
    let part = part_path(out);
    if let Some(dir) = part.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Không tạo được thư mục tạm: {e}"))?;
    }
    let result = write_all(source, &part, plan, bitrate, progress, cancel);
    let first_frame = match result {
        Ok(frame) => frame,
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
    };
    // The planned name may have been taken while exporting: never replace that file.
    let out = place(source, &part, out)?;
    let (w, h) = plan.out_size();
    if let Some(frame) = first_frame {
        crate::pipeline::save_thumbnail(&frame, w, h, &thumb_path(&out));
    }
    write_meta(&out, &MediaMeta::from_frames(plan.frames(), plan.fps(), w, h));
    Ok(out)
}

/// Encodes every planned frame into `part`; returns the first output frame for the thumbnail.
fn write_all(
    source: &Path,
    part: &Path,
    plan: &Plan,
    bitrate: u32,
    progress: &mut dyn FnMut(u64, u64),
    cancel: &AtomicBool,
) -> Result<Option<Vec<u8>>, String> {
    let (mut video, info) = VideoDecoder::open(source)?;
    let mut audio = AudioDecoder::open(source)?.map(|dec| AudioPump::new(dec, plan.cuts().clone()));
    let (out_w, out_h) = plan.out_size();
    let fps = plan.fps();
    let cfg = EncoderConfig { path: part.to_path_buf(), width: out_w, height: out_h, fps, video_bitrate: bitrate, audio: audio.is_some() };
    let mut enc = Mp4Encoder::new(&cfg)?;
    let mut compositor: Option<Compositor> = None;
    let total = plan.frames();
    let mut first = None;
    for n in 0..total {
        if cancel.load(Ordering::Relaxed) {
            return Err("Đã hủy".to_string());
        }
        let Some(fp) = plan.frame(n) else { break };
        let src = video.frame_at(fp.src_ms)?;
        // Same size, no zoom, no frame layout and nothing to draw: the encoder crops an odd
        // source edge by itself.
        let zoomed = fp.camera.scale > 1.0001;
        let layout = plan.layout();
        let plain = !layout.decorated && layout.fill_aspect.is_none();
        let direct = plain && !zoomed && fp.draws.is_empty() && info.width >= out_w && info.height >= out_h && info.width - out_w <= 1 && info.height - out_h <= 1;
        let (frame, stride): (&[u8], u32) = if direct {
            (src, info.width * 4)
        } else {
            if compositor.is_none() {
                compositor = Some(Compositor::new(out_w, out_h, Device::Hardware)?);
            }
            let c = compositor.as_mut().expect("just created");
            (c.render(src, info.width, info.height, Some(fp.view), Some(layout), &fp.draws)?, out_w * 4)
        };
        if first.is_none() {
            first = Some(tight(frame, stride, out_w, out_h));
        }
        enc.write_video(frame, stride, n as i64 * 10_000_000 / fps as i64)?;
        if let Some(pump) = audio.as_mut() {
            pump.write_until(&mut enc, fp.out_ms as f64 + AUDIO_LEAD_MS)?;
        }
        if (n + 1) % PROGRESS_EVERY == 0 || n + 1 == total {
            progress(n + 1, total);
        }
    }
    if let Some(pump) = audio.as_mut() {
        pump.write_until(&mut enc, f64::INFINITY)?;
    }
    enc.finish()?;
    Ok(first)
}

fn tight(frame: &[u8], stride: u32, w: u32, h: u32) -> Vec<u8> {
    let row = w as usize * 4;
    frame.chunks(stride as usize).take(h as usize).flat_map(|r| r[..row].iter().copied()).collect()
}

/// Feeds decoded audio to the encoder in output time, dropping removed spans and fading
/// around joins.
struct AudioPump {
    dec: AudioDecoder,
    cuts: Cuts,
    /// Decoded samples not yet written: source ms of the first frame, interleaved stereo.
    pending: Option<(f64, Vec<f32>)>,
    ended: bool,
}

impl AudioPump {
    fn new(dec: AudioDecoder, cuts: Cuts) -> Self {
        AudioPump { dec, cuts, pending: None, ended: false }
    }

    /// Writes every kept sample whose output time is before `until_out_ms`.
    fn write_until(&mut self, enc: &mut Mp4Encoder, until_out_ms: f64) -> Result<(), String> {
        let ms_per_frame = 1000.0 / AUDIO_SAMPLE_RATE as f64;
        let ch = AUDIO_CHANNELS as usize;
        loop {
            if self.pending.is_none() {
                if self.ended {
                    return Ok(());
                }
                self.pending = self.dec.next_chunk()?;
                if self.pending.is_none() {
                    self.ended = true;
                    return Ok(());
                }
            }
            let (start, pcm) = self.pending.take().expect("checked above");
            let frames = pcm.len() / ch;
            // Stop before a chunk whose kept part starts beyond the window.
            let first_out = (0..frames).find_map(|i| self.cuts.to_output_f64(start + i as f64 * ms_per_frame));
            if first_out.is_some_and(|o| o >= until_out_ms) {
                self.pending = Some((start, pcm));
                return Ok(());
            }
            // Split the chunk into runs that are contiguous in output time.
            let mut run: Vec<f32> = Vec::new();
            let mut run_start: Option<f64> = None;
            let mut expected = 0.0;
            for i in 0..frames {
                let s = start + i as f64 * ms_per_frame;
                match self.cuts.to_output_f64(s) {
                    Some(o) => {
                        if run_start.is_some() && (o - expected).abs() > ms_per_frame / 2.0 {
                            write_run(enc, run_start.take(), &mut run)?;
                        }
                        if run_start.is_none() {
                            run_start = Some(o);
                        }
                        let g = self.cuts.join_gain(s);
                        run.extend(pcm[i * ch..(i + 1) * ch].iter().map(|v| v * g));
                        expected = o + ms_per_frame;
                    }
                    None => write_run(enc, run_start.take(), &mut run)?,
                }
            }
            write_run(enc, run_start.take(), &mut run)?;
        }
    }
}

fn write_run(enc: &mut Mp4Encoder, start_out_ms: Option<f64>, run: &mut Vec<f32>) -> Result<(), String> {
    if let Some(start) = start_out_ms
        && !run.is_empty()
    {
        enc.write_audio(run, (start * 10_000.0).round() as i64)?;
    }
    run.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::VideoDecoder;
    use crate::encoder::probe;
    use crate::test_media::*;
    use hp_core::cut::Span;
    use hp_core::edit::EditDoc;
    use hp_core::library::DATA_DIR;
    use hp_core::plan::Source;
    use std::sync::atomic::Ordering;

    fn plan_for(path: &Path, cuts: Vec<Span>) -> Plan {
        let (_, info) = VideoDecoder::open(path).unwrap();
        let src = Source { width: info.width, height: info.height, duration_ms: info.duration_ms, fps: info.fps };
        Plan::new(src, &EditDoc { cuts, ..EditDoc::default() }, None)
    }

    fn export(source: &Path, plan: &Plan) -> Result<PathBuf, String> {
        let out = export_path(source);
        run_export(source, &out, plan, 2_000_000, &mut |_, _| {}, &AtomicBool::new(false))
    }

    #[test]
    fn export_drops_the_removed_span() {
        let source = temp_path("exp-cut", "a.mp4");
        make_sample(&source, 3, 320, 240, true);
        let plan = plan_for(&source, vec![Span { start_ms: 1000, end_ms: 2000 }]);
        let out = export(&source, &plan).unwrap();
        let (frames, last, audio) = probe(&out).unwrap();
        assert!((59..=61).contains(&frames), "{frames} frames");
        assert!(last < 20_000_000, "last frame at {last}");
        assert!(audio > 0);
        let (mut dec, _) = VideoDecoder::open(&out).unwrap();
        assert!(near(centre_rgb(dec.frame_at(500).unwrap(), 320, 240), [255, 0, 0]));
        assert!(near(centre_rgb(dec.frame_at(1500).unwrap(), 320, 240), [0, 0, 255]));
        let mut ms = 0;
        while ms < 2000 {
            assert!(!near(centre_rgb(dec.frame_at(ms).unwrap(), 320, 240), [0, 255, 0]), "green frame at {ms} ms");
            ms += 33;
        }
    }

    #[test]
    fn export_draws_click_effects() {
        use hp_core::effects::build_effects;
        use hp_core::input_log::{Button, Event, ParsedLog};
        let source = temp_path("exp-fx", "a.mp4");
        make_sample(&source, 1, 640, 480, false);
        let (_, info) = VideoDecoder::open(&source).unwrap();
        let log = ParsedLog {
            header: None,
            events: vec![Event::Down { t: 0, b: Button::Right, x: 320, y: 240 }, Event::Up { t: 900, b: Button::Right, x: 320, y: 240 }],
        };
        let src = Source { width: info.width, height: info.height, duration_ms: info.duration_ms, fps: info.fps };
        let plan = Plan::new(src, &EditDoc::default(), Some(build_effects(&log, 640, 480)));
        let out = export(&source, &plan).unwrap();
        let (mut dec, _) = VideoDecoder::open(&out).unwrap();
        // The red frame gets a blue hold dot at the click point.
        let [_, _, b] = centre_rgb(dec.frame_at(500).unwrap(), 640, 480);
        assert!(b > 80, "blue from the right-click dot, got {b}");
    }

    #[test]
    fn a_file_that_appears_during_export_is_never_overwritten() {
        let source = temp_path("exp-race", "a.mp4");
        make_sample(&source, 1, 320, 240, false);
        let planned = export_path(&source);
        // Someone takes the planned name while the export runs (e.g. a rename in the library).
        std::fs::write(&planned, b"precious").unwrap();
        let written = run_export(&source, &planned, &plan_for(&source, vec![]), 2_000_000, &mut |_, _| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(std::fs::read(&planned).unwrap(), b"precious");
        assert_eq!(written, source.with_file_name("a_edit_2.mp4"));
        assert!(written.exists());
    }

    fn zoomed(cuts: Vec<Span>, start_ms: i64, end_ms: i64) -> EditDoc {
        use hp_core::zoom::{ZoomDoc, ZoomSegment};
        let segments = vec![ZoomSegment { start_ms, end_ms, scale: 2.0, center: Some((0.25, 0.25)), manual: true, ..ZoomSegment::default() }];
        EditDoc { cuts, zoom: ZoomDoc { enabled: true, segments, ..ZoomDoc::default() }, ..EditDoc::default() }
    }

    fn plan_doc(path: &Path, doc: &EditDoc) -> Plan {
        let (_, info) = VideoDecoder::open(path).unwrap();
        let src = Source { width: info.width, height: info.height, duration_ms: info.duration_ms, fps: info.fps };
        Plan::new(src, doc, None)
    }

    #[test]
    fn zoomed_export_shows_the_zoomed_area() {
        let source = temp_path("exp-zoom", "a.mp4");
        make_quadrants(&source, 2, 320, 240);
        let out = export(&source, &plan_doc(&source, &zoomed(vec![], 0, 2000))).unwrap();
        let (mut dec, _) = VideoDecoder::open(&out).unwrap();
        let f = dec.frame_at(1800).unwrap();
        assert!(near(centre_rgb(f, 320, 240), [255, 0, 0]), "centre shows the zoomed top-left: {:?}", centre_rgb(f, 320, 240));
        // At 2x on the top-left quarter, the whole frame is red.
        let i = ((230 * 320 + 310) * 4) as usize;
        assert!(near([f[i + 2], f[i + 1], f[i]], [255, 0, 0]), "bottom-right corner is still red: {:?}", [f[i + 2], f[i + 1], f[i]]);
    }

    fn panning(cuts: Vec<Span>) -> EditDoc {
        use hp_core::zoom::{Speed, ZoomDoc, ZoomKey, ZoomSegment};
        let seg = ZoomSegment {
            start_ms: 0,
            end_ms: 4000,
            scale: 2.0,
            center: Some((0.25, 0.25)),
            manual: true,
            keys: vec![ZoomKey { t_ms: 2000, cx: 0.75, cy: 0.75, scale: 2.0 }],
            speed: Speed::Instant,
        };
        EditDoc { cuts, zoom: ZoomDoc { enabled: true, segments: vec![seg], ..ZoomDoc::default() }, ..EditDoc::default() }
    }

    #[test]
    fn keyframes_pan_the_export() {
        let source = temp_path("exp-keys", "a.mp4");
        make_quadrants(&source, 4, 320, 240);
        let out = export(&source, &plan_doc(&source, &panning(vec![]))).unwrap();
        let (mut dec, _) = VideoDecoder::open(&out).unwrap();
        let early = centre_rgb(dec.frame_at(1000).unwrap(), 320, 240);
        assert!(near(early, [255, 0, 0]), "top-left before the keyframe: {early:?}");
        let late = centre_rgb(dec.frame_at(3000).unwrap(), 320, 240);
        assert!(near(late, [255, 255, 255]), "bottom-right after it: {late:?}");
    }

    #[test]
    fn keyframes_inside_a_cut_export() {
        let source = temp_path("exp-keys-cut", "a.mp4");
        make_quadrants(&source, 4, 320, 240);
        let out = export(&source, &plan_doc(&source, &panning(vec![Span { start_ms: 1500, end_ms: 2500 }]))).unwrap();
        let (frames, _, _) = probe(&out).unwrap();
        assert!((89..=91).contains(&frames), "3 s kept: {frames}");
        let (mut dec, _) = VideoDecoder::open(&out).unwrap();
        let late = centre_rgb(dec.frame_at(2000).unwrap(), 320, 240);
        assert!(near(late, [255, 255, 255]), "source 3000 ms (output 2000 ms) is past the keyframe: {late:?}");
    }

    #[test]
    fn zoom_inside_a_cut_exports() {
        let source = temp_path("exp-zoom-cut", "a.mp4");
        make_quadrants(&source, 2, 320, 240);
        let out = export(&source, &plan_doc(&source, &zoomed(vec![Span { start_ms: 500, end_ms: 1500 }], 600, 1400))).unwrap();
        let (frames, _, _) = probe(&out).unwrap();
        assert!((29..=31).contains(&frames), "{frames}");
    }

    #[test]
    fn tall_export_has_the_frame_size_and_background() {
        use hp_core::layout::{Aspect, FrameDoc};
        let source = temp_path("exp-tall", "a.mp4");
        make_sample(&source, 1, 320, 180, false);
        let doc = EditDoc { frame: FrameDoc { aspect: Aspect::Tall, background: "white".into(), ..FrameDoc::default() }, ..EditDoc::default() };
        let out = export(&source, &plan_doc(&source, &doc)).unwrap();
        let (mut dec, info) = VideoDecoder::open(&out).unwrap();
        assert_eq!((info.width, info.height), (100, 180), "9:16 of a 180 px tall source (101.25 made even)");
        let f = dec.frame_at(500).unwrap();
        let i = (2 * 100 + 50) * 4;
        let top = [f[i + 2], f[i + 1], f[i]];
        assert!(near(top, [255, 255, 255]), "white bar above the video: {top:?}");
        assert!(near(centre_rgb(f, 100, 180), [255, 0, 0]), "red video in the middle");
    }

    #[test]
    fn fill_export_crops() {
        use hp_core::layout::{Aspect, Fit, FrameDoc};
        let source = temp_path("exp-fill", "a.mp4");
        make_quadrants(&source, 1, 320, 180);
        let doc = EditDoc { frame: FrameDoc { aspect: Aspect::Square, fit: Fit::Fill, ..FrameDoc::default() }, ..EditDoc::default() };
        let out = export(&source, &plan_doc(&source, &doc)).unwrap();
        let (mut dec, info) = VideoDecoder::open(&out).unwrap();
        assert_eq!((info.width, info.height), (180, 180));
        let f = dec.frame_at(500).unwrap();
        let px = |x: u32, y: u32| {
            let i = ((y * 180 + x) * 4) as usize;
            [f[i + 2], f[i + 1], f[i]]
        };
        assert!(near(px(20, 20), [255, 0, 0]) && near(px(160, 20), [0, 255, 0]), "the centre square of the source: {:?} {:?}", px(20, 20), px(160, 20));
    }

    fn gif_frames(path: &Path) -> (u16, u16, usize) {
        let mut dec = gif::DecodeOptions::new().read_info(std::fs::File::open(path).unwrap()).unwrap();
        let (w, h) = (dec.width(), dec.height());
        let mut n = 0;
        while dec.read_next_frame().unwrap().is_some() {
            n += 1;
        }
        (w, h, n)
    }

    #[test]
    fn gif_has_the_frames_and_size() {
        let source = temp_path("exp-gif", "a.mp4");
        make_sample(&source, 1, 320, 240, true);
        let plan = plan_for(&source, vec![]).for_gif(10, 160);
        let out = export_path_ext(&source, "gif");
        let written = run_export_gif(&source, &out, &plan, &mut |_, _| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(written, source.with_file_name("a_edit.gif"));
        assert_eq!(gif_frames(&written), (160, 120, 10));
    }

    #[test]
    fn gif_of_a_silent_video() {
        let source = temp_path("exp-gif-silent", "a.mp4");
        make_sample(&source, 1, 320, 240, false);
        let plan = plan_for(&source, vec![]).for_gif(15, 120);
        let out = run_export_gif(&source, &export_path_ext(&source, "gif"), &plan, &mut |_, _| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(gif_frames(&out).2, 15);
    }

    #[test]
    fn gif_names_do_not_overwrite() {
        let source = temp_path("exp-gif-name", "a.mp4");
        std::fs::write(source.with_file_name("a_edit.gif"), b"x").unwrap();
        assert_eq!(export_path_ext(&source, "gif"), source.with_file_name("a_edit_2.gif"));
        assert_eq!(hp_core::library::kind_of(&source.with_file_name("a_edit.gif")), Some(hp_core::library::Kind::Image));
    }

    #[test]
    fn export_never_touches_the_source() {
        let source = temp_path("exp-src", "a.mp4");
        make_sample(&source, 1, 320, 240, false);
        let before = std::fs::metadata(&source).unwrap();
        export(&source, &plan_for(&source, vec![])).unwrap();
        let after = std::fs::metadata(&source).unwrap();
        assert_eq!(before.len(), after.len());
        assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    }

    #[test]
    fn export_name_does_not_overwrite() {
        let source = temp_path("exp-name", "a.mp4");
        assert_eq!(export_path(&source), source.with_file_name("a_edit.mp4"));
        std::fs::write(source.with_file_name("a_edit.mp4"), b"x").unwrap();
        assert_eq!(export_path(&source), source.with_file_name("a_edit_2.mp4"));
    }

    #[test]
    fn export_writes_thumbnail_and_meta() {
        let source = temp_path("exp-meta", "a.mp4");
        make_sample(&source, 1, 320, 240, false);
        let out = export(&source, &plan_for(&source, vec![])).unwrap();
        assert!(hp_core::library::thumb_path(&out).exists());
        assert!(hp_core::library::meta_path(&out).exists());
    }

    #[test]
    fn cancelled_export_leaves_no_file() {
        let source = temp_path("exp-cancel", "a.mp4");
        make_sample(&source, 1, 320, 240, false);
        let out = export_path(&source);
        let cancel = AtomicBool::new(true);
        let r = run_export(&source, &out, &plan_for(&source, vec![]), 2_000_000, &mut |_, _| {}, &cancel);
        assert!(r.is_err());
        assert!(!cancel.load(Ordering::Relaxed) || !out.exists());
        let data = source.parent().unwrap().join(DATA_DIR);
        let parts = std::fs::read_dir(&data).map(|d| d.filter_map(|e| e.ok()).filter(|e| e.path().extension().is_some_and(|x| x == "part")).count()).unwrap_or(0);
        assert_eq!(parts, 0);
    }

    #[test]
    fn video_without_audio_exports() {
        let source = temp_path("exp-noaudio", "a.mp4");
        make_sample(&source, 1, 320, 240, false);
        let out = export(&source, &plan_for(&source, vec![])).unwrap();
        let (frames, _, audio) = probe(&out).unwrap();
        assert!(frames >= 29);
        assert_eq!(audio, 0);
    }

    #[test]
    fn vietnamese_names_export() {
        let source = temp_path("exp-vi", "bài giảng.mp4");
        make_sample(&source, 1, 320, 240, false);
        let out = export(&source, &plan_for(&source, vec![])).unwrap();
        assert_eq!(out.file_name().unwrap().to_string_lossy(), "bài giảng_edit.mp4");
        assert!(out.exists());
    }

    #[test]
    fn progress_reaches_the_total() {
        let source = temp_path("exp-progress", "a.mp4");
        make_sample(&source, 1, 320, 240, false);
        let plan = plan_for(&source, vec![]);
        let mut last = (0, 0);
        run_export(&source, &export_path(&source), &plan, 2_000_000, &mut |d, t| last = (d, t), &AtomicBool::new(false)).unwrap();
        assert_eq!(last, (plan.frames(), plan.frames()));
    }
}
