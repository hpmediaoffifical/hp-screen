//! Reads videos back with the Media Foundation Source Reader: BGRA frames for export and
//! the editor, and 48 kHz stereo float audio.
use crate::encoder::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, init_thread};
use hp_core::mix::{Resampler, to_stereo};
use serde::Serialize;
use std::path::Path;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Variant::VT_I8;
use windows::core::{GUID, HSTRING, Interface};

const VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
const AUDIO: u32 = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
/// Jumps further ahead than this seek instead of decoding every frame in between.
const SEEK_AHEAD_MS: i64 = 1000;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration_ms: i64,
    pub fps: u32,
    pub has_audio: bool,
}

fn err(context: &str) -> impl Fn(windows::core::Error) -> String + '_ {
    move |e| format!("{context}: {e}")
}

fn unpack(v: u64) -> (u32, u32) {
    ((v >> 32) as u32, v as u32)
}

unsafe fn open_reader(path: &Path, video_processing: bool) -> windows::core::Result<IMFSourceReader> {
    unsafe {
        let mut attrs = None;
        MFCreateAttributes(&mut attrs, 1)?;
        let attrs = attrs.ok_or_else(windows::core::Error::empty)?;
        if video_processing {
            attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)?;
        }
        MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), &attrs)
    }
}

unsafe fn set_output(reader: &IMFSourceReader, stream: u32, major: &GUID, subtype: &GUID) -> windows::core::Result<()> {
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, major)?;
        t.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        reader.SetCurrentMediaType(stream, None, &t)
    }
}

/// Duration in ms from the container, or 0 when it does not say.
unsafe fn container_duration_ms(reader: &IMFSourceReader) -> i64 {
    unsafe {
        match reader.GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION) {
            Ok(v) => (v.Anonymous.Anonymous.Anonymous.uhVal / 10_000) as i64,
            Err(_) => 0,
        }
    }
}

/// Duration by walking the compressed video samples (fragmented files may not state it).
unsafe fn scanned_duration_ms(path: &Path, frame_100ns: i64) -> windows::core::Result<i64> {
    unsafe {
        let reader = open_reader(path, false)?;
        reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
        reader.SetStreamSelection(VIDEO, true)?;
        let mut last: Option<i64> = None;
        loop {
            let (mut flags, mut time, mut sample) = (0u32, 0i64, None);
            reader.ReadSample(VIDEO, 0, None, Some(&mut flags), Some(&mut time), Some(&mut sample))?;
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                break;
            }
            if sample.is_some() {
                last = Some(last.map_or(time, |l| l.max(time)));
            }
        }
        Ok(last.map_or(0, |t| (t + frame_100ns) / 10_000))
    }
}

fn seek(reader: &IMFSourceReader, ms: i64) -> windows::core::Result<()> {
    let mut pos = PROPVARIANT::default();
    // SAFETY: a VT_I8 PROPVARIANT holds a plain integer; nothing needs clearing.
    unsafe {
        let inner = &mut pos.Anonymous.Anonymous;
        inner.vt = VT_I8;
        inner.Anonymous.hVal = ms.max(0) * 10_000;
        reader.SetCurrentPosition(&GUID::zeroed(), &pos)
    }
}

type TimedFrame = (i64, Vec<u8>);

pub struct VideoDecoder {
    reader: IMFSourceReader,
    width: u32,
    height: u32,
    /// Picture origin inside the decoded (possibly padded) buffer.
    offset: (u32, u32),
    /// Row pitch to use when a buffer is not 2D (negative: rows stored bottom-up).
    default_stride: i32,
    /// The frame handed out last, with its time in ms.
    current: Option<TimedFrame>,
    /// A frame already read but later than what was asked for.
    pending: Option<TimedFrame>,
    ended: bool,
}

// SAFETY: the Source Reader is free-threaded; every thread using it calls `init_thread`.
unsafe impl Send for VideoDecoder {}

impl VideoDecoder {
    pub fn open(path: &Path) -> Result<(VideoDecoder, VideoInfo), String> {
        init_thread();
        // SAFETY: Media Foundation is started; every out pointer is valid for its call.
        unsafe {
            let reader = open_reader(path, true).map_err(err("Không mở được video"))?;
            let native = reader.GetNativeMediaType(VIDEO, 0).map_err(err("Video không có hình"))?;
            let (num, den) = native.GetUINT64(&MF_MT_FRAME_RATE).map(unpack).unwrap_or((30, 1));
            let fps = fps_from_ratio(num, den);
            set_output(&reader, VIDEO, &MFMediaType_Video, &MFVideoFormat_RGB32).map_err(err("Không giải mã được video"))?;
            let out = reader.GetCurrentMediaType(VIDEO).map_err(err("Không giải mã được video"))?;
            let (fw, fh) = unpack(out.GetUINT64(&MF_MT_FRAME_SIZE).map_err(err("Video không rõ kích thước"))?);
            let (offset, width, height) = match aperture(&native).or_else(|| aperture(&out)) {
                Some((x, y, w, h)) if x.checked_add(w).is_some_and(|r| r <= fw) && y.checked_add(h).is_some_and(|b| b <= fh) => ((x, y), w, h),
                _ => ((0, 0), fw, fh),
            };
            let default_stride = out.GetUINT32(&MF_MT_DEFAULT_STRIDE).map(|s| s as i32).unwrap_or(fw as i32 * 4);
            let has_audio = reader.GetNativeMediaType(AUDIO, 0).is_ok();
            if has_audio {
                let _ = reader.SetStreamSelection(AUDIO, false);
            }
            let mut duration_ms = container_duration_ms(&reader);
            if duration_ms <= 0 {
                duration_ms = scanned_duration_ms(path, 10_000_000 / fps as i64).unwrap_or(0);
            }
            let info = VideoInfo { width, height, duration_ms, fps, has_audio };
            let dec = VideoDecoder { reader, width, height, offset, default_stride, current: None, pending: None, ended: false };
            Ok((dec, info))
        }
    }

    /// The last frame whose time is at or before `src_ms` (the first frame when `src_ms` is
    /// before it): top-down BGRA, `width * 4` bytes per row.
    pub fn frame_at(&mut self, src_ms: i64) -> Result<&[u8], String> {
        let jump = match &self.current {
            Some((t, _)) => src_ms < *t || src_ms > *t + SEEK_AHEAD_MS,
            None => false,
        };
        if jump {
            seek(&self.reader, src_ms).map_err(err("Không tua được video"))?;
            self.current = None;
            self.pending = None;
            self.ended = false;
        }
        loop {
            if self.pending.is_none() && !self.ended {
                self.pending = self.read_frame()?;
                self.ended = self.pending.is_none();
            }
            match self.pending.take() {
                Some(next) if self.current.is_none() || next.0 <= src_ms => self.current = Some(next),
                other => {
                    self.pending = other;
                    break;
                }
            }
        }
        self.current.as_ref().map(|(_, f)| f.as_slice()).ok_or_else(|| "Video không có khung hình nào".to_string())
    }

    fn read_frame(&mut self) -> Result<Option<TimedFrame>, String> {
        // SAFETY: out pointers are valid for the call.
        unsafe {
            loop {
                let (mut flags, mut time, mut sample) = (0u32, 0i64, None);
                self.reader
                    .ReadSample(VIDEO, 0, None, Some(&mut flags), Some(&mut time), Some(&mut sample))
                    .map_err(err("Lỗi khi đọc video"))?;
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    return Ok(None);
                }
                let Some(sample) = sample else { continue };
                let buffer = sample.ConvertToContiguousBuffer().map_err(err("Lỗi khi đọc video"))?;
                let frame = self.copy_picture(&buffer)?;
                return Ok(Some((time / 10_000, frame)));
            }
        }
    }

    /// Copies the visible picture out of a decoded buffer as tight top-down BGRA.
    unsafe fn copy_picture(&self, buffer: &IMFMediaBuffer) -> Result<Vec<u8>, String> {
        // SAFETY: forwarded; `buffer` is a live media buffer.
        unsafe { copy_bgra(buffer, self.width, self.height, self.offset, self.default_stride) }
    }
}

/// Copies a `width`×`height` BGRA picture starting at `offset` out of a media buffer as tight
/// top-down rows. The buffer's geometry comes from the file or device, so every row is
/// bounds-checked before it is read. `default_stride` is used when the buffer is not 2D.
pub(crate) unsafe fn copy_bgra(buffer: &IMFMediaBuffer, width: u32, height: u32, offset: (u32, u32), default_stride: i32) -> Result<Vec<u8>, String> {
    unsafe {
        let row = width as usize * 4;
        let rows = height as usize;
        let (x0, y0) = (offset.0 as usize * 4, offset.1 as usize);
        let mut out = vec![0u8; row * rows];
        let mut copy = |start: *const u8, len: usize, base: isize, pitch: isize| -> Result<(), String> {
            if !rows_in_bounds(len, base, pitch, x0, y0, rows, row) {
                return Err("Khung hình không hợp lệ".to_string());
            }
            for (y, dst) in out.chunks_exact_mut(row).enumerate() {
                let src = start.offset(base + (y + y0) as isize * pitch + x0 as isize);
                std::ptr::copy_nonoverlapping(src, dst.as_mut_ptr(), row);
            }
            Ok(())
        };
        if let Ok(b2) = buffer.cast::<IMF2DBuffer2>() {
            let (mut scan0, mut pitch, mut start, mut len) = (std::ptr::null_mut(), 0i32, std::ptr::null_mut(), 0u32);
            b2.Lock2DSize(MF2DBuffer_LockFlags_Read, &mut scan0, &mut pitch, &mut start, &mut len).map_err(err("Lỗi khi đọc khung hình"))?;
            let result = copy(start, len as usize, scan0.offset_from(start), pitch as isize);
            let _ = b2.Unlock2D();
            result?;
        } else {
            let (mut data, mut len) = (std::ptr::null_mut(), 0u32);
            buffer.Lock(&mut data, None, Some(&mut len)).map_err(err("Lỗi khi đọc khung hình"))?;
            let pitch = default_stride as isize;
            // Bottom-up rows: the top row is the last one in memory.
            let base = if pitch < 0 { len as isize + pitch } else { 0 };
            let result = copy(data, len as usize, base, pitch);
            let _ = buffer.Unlock();
            result?;
        }
        Ok(out)
    }
}

/// Visible picture area (x, y, w, h) when the media type says the frame is padded.
unsafe fn aperture(t: &IMFMediaType) -> Option<(u32, u32, u32, u32)> {
    unsafe {
        for key in [&MF_MT_MINIMUM_DISPLAY_APERTURE, &MF_MT_GEOMETRIC_APERTURE] {
            let mut area = MFVideoArea::default();
            let bytes = std::slice::from_raw_parts_mut(&mut area as *mut MFVideoArea as *mut u8, std::mem::size_of::<MFVideoArea>());
            if t.GetBlob(key, bytes, None).is_ok() {
                let (x, y) = (area.OffsetX.value, area.OffsetY.value);
                let (w, h) = (area.Area.cx, area.Area.cy);
                if x < 0 || y < 0 || w <= 0 || h <= 0 {
                    return None;
                }
                return Some((x as u32, y as u32, w as u32, h as u32));
            }
        }
        None
    }
}

pub struct AudioDecoder {
    reader: IMFSourceReader,
    rate: u32,
    channels: usize,
    resampler: Resampler,
}

// SAFETY: as for VideoDecoder.
unsafe impl Send for AudioDecoder {}

impl AudioDecoder {
    /// `None` when the file has no audio track.
    pub fn open(path: &Path) -> Result<Option<AudioDecoder>, String> {
        init_thread();
        // SAFETY: Media Foundation is started; out pointers are valid.
        unsafe {
            let reader = open_reader(path, false).map_err(err("Không mở được video"))?;
            if reader.GetNativeMediaType(AUDIO, 0).is_err() {
                return Ok(None);
            }
            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false).map_err(err("Không đọc được âm thanh"))?;
            reader.SetStreamSelection(AUDIO, true).map_err(err("Không đọc được âm thanh"))?;
            set_output(&reader, AUDIO, &MFMediaType_Audio, &MFAudioFormat_Float).map_err(err("Không giải mã được âm thanh"))?;
            let t = reader.GetCurrentMediaType(AUDIO).map_err(err("Không giải mã được âm thanh"))?;
            let rate = t.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND).unwrap_or(AUDIO_SAMPLE_RATE);
            let channels = t.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).unwrap_or(AUDIO_CHANNELS) as usize;
            Ok(Some(AudioDecoder { reader, rate, channels, resampler: Resampler::new(rate, AUDIO_SAMPLE_RATE) }))
        }
    }

    /// Next decoded chunk: (source ms of its first sample, interleaved 48 kHz stereo floats).
    pub fn next_chunk(&mut self) -> Result<Option<(f64, Vec<f32>)>, String> {
        // SAFETY: out pointers are valid; the buffer is only read while locked.
        unsafe {
            loop {
                let (mut flags, mut time, mut sample) = (0u32, 0i64, None);
                self.reader
                    .ReadSample(AUDIO, 0, None, Some(&mut flags), Some(&mut time), Some(&mut sample))
                    .map_err(err("Lỗi khi đọc âm thanh"))?;
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    return Ok(None);
                }
                let Some(sample) = sample else { continue };
                let buffer = sample.ConvertToContiguousBuffer().map_err(err("Lỗi khi đọc âm thanh"))?;
                let (mut data, mut len) = (std::ptr::null_mut(), 0u32);
                buffer.Lock(&mut data, None, Some(&mut len)).map_err(err("Lỗi khi đọc âm thanh"))?;
                // Read as bytes: the buffer is not guaranteed to be aligned for f32.
                let bytes = std::slice::from_raw_parts(data as *const u8, len as usize);
                let pcm: Vec<f32> = bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
                let _ = buffer.Unlock();
                let out = to_48k_stereo(&pcm, self.rate, self.channels, &mut self.resampler);
                return Ok(Some((time as f64 / 10_000.0, out)));
            }
        }
    }
}

/// True when every byte of `rows` rows of `row` bytes, starting `x0` bytes into row `y0`,
/// lies inside a buffer of `len` bytes whose row 0 starts at byte `base` and whose rows are
/// `pitch` bytes apart (negative: stored bottom-up).
fn rows_in_bounds(len: usize, base: isize, pitch: isize, x0: usize, y0: usize, rows: usize, row: usize) -> bool {
    if rows == 0 || pitch == 0 || x0.saturating_add(row) > pitch.unsigned_abs() {
        return false;
    }
    let start_of = |y: usize| -> Option<isize> { base.checked_add((y as isize).checked_mul(pitch)?)?.checked_add(x0 as isize) };
    let Some(last_y) = y0.checked_add(rows - 1) else { return false };
    [y0, last_y].into_iter().all(|y| match start_of(y) {
        Some(start) => start >= 0 && (start as usize).saturating_add(row) <= len,
        None => false,
    })
}

/// Integer frame rate from a ratio: 30000/1001 gives 30.
fn fps_from_ratio(num: u32, den: u32) -> u32 {
    if den == 0 {
        return 1;
    }
    ((num as f64 / den as f64).round() as u32).max(1)
}

/// Converts interleaved samples of any rate and channel count to 48 kHz stereo.
fn to_48k_stereo(pcm: &[f32], rate: u32, channels: usize, rs: &mut Resampler) -> Vec<f32> {
    let stereo = if channels == 2 { pcm.to_vec() } else { to_stereo(pcm, channels) };
    if rate == AUDIO_SAMPLE_RATE { stereo } else { rs.process(&stereo) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_media::*;

    #[test]
    fn row_bounds_are_checked_before_copying() {
        // 4 rows of 16 bytes, top-down.
        assert!(rows_in_bounds(64, 0, 16, 0, 0, 4, 16));
        assert!(rows_in_bounds(64, 0, 16, 4, 1, 3, 12));
        assert!(!rows_in_bounds(64, 0, 16, 0, 1, 4, 16), "last row past the end");
        assert!(!rows_in_bounds(64, 0, 16, 8, 0, 1, 12), "row spills into the next one");
        assert!(!rows_in_bounds(64, 0, 0, 0, 0, 4, 16), "zero pitch");
        // Bottom-up: row 0 is the last 16 bytes.
        assert!(rows_in_bounds(64, 48, -16, 0, 0, 4, 16));
        assert!(!rows_in_bounds(64, 48, -16, 0, 0, 5, 16), "row before the start");
        assert!(!rows_in_bounds(64, 60, -16, 0, 0, 1, 16), "first row past the end");
    }

    #[test]
    fn fps_is_rounded_from_the_ratio() {
        assert_eq!(fps_from_ratio(30000, 1001), 30);
        assert_eq!(fps_from_ratio(60, 1), 60);
        assert_eq!(fps_from_ratio(0, 0), 1);
    }

    #[test]
    fn mono_44k_becomes_48k_stereo() {
        let mut rs = Resampler::new(44_100, 48_000);
        let out = to_48k_stereo(&vec![0.1; 44_100], 44_100, 1, &mut rs);
        assert!((out.len() as i64 - 96_000).abs() <= 200, "{}", out.len());
    }

    #[test]
    fn opens_a_recording_and_reports_its_size() {
        let path = temp_path("dec-open", "a.mp4");
        make_sample(&path, 3, 320, 240, true);
        let (_, info) = VideoDecoder::open(&path).unwrap();
        assert_eq!((info.width, info.height, info.fps, info.has_audio), (320, 240, 30, true));
        assert!((2900..=3100).contains(&info.duration_ms), "{}", info.duration_ms);
    }

    #[test]
    fn frame_at_returns_the_frame_for_that_time() {
        let path = temp_path("dec-frames", "a.mp4");
        make_sample(&path, 3, 320, 240, false);
        let (mut dec, _) = VideoDecoder::open(&path).unwrap();
        for (ms, second) in [(500, 0), (1500, 1), (2500, 2), (500, 0), (2990, 2)] {
            let f = dec.frame_at(ms).unwrap();
            assert_eq!(f.len(), 320 * 240 * 4);
            let rgb = centre_rgb(f, 320, 240);
            assert!(near(rgb, colour_for_second(second)), "at {ms} ms got {rgb:?}");
        }
    }

    #[test]
    fn sizes_that_are_not_multiples_of_16_decode_at_their_true_size() {
        let path = temp_path("dec-odd", "a.mp4");
        make_sample(&path, 1, 330, 202, false);
        let (mut dec, info) = VideoDecoder::open(&path).unwrap();
        assert_eq!((info.width, info.height), (330, 202));
        let f = dec.frame_at(100).unwrap();
        assert_eq!(f.len(), 330 * 202 * 4);
        // The last row must be picture, not padding.
        let last = &f[(201 * 330 * 4) as usize..];
        assert!(near([last[2], last[1], last[0]], [255, 0, 0]), "{:?}", &last[..4]);
    }

    #[test]
    fn audio_decodes_to_48k_stereo() {
        let path = temp_path("dec-audio", "a.mp4");
        make_sample(&path, 3, 320, 240, true);
        let mut dec = AudioDecoder::open(&path).unwrap().expect("has audio");
        let mut total = 0usize;
        while let Some((_, pcm)) = dec.next_chunk().unwrap() {
            total += pcm.len();
        }
        let frames = total as i64 / 2;
        assert!((frames - 144_000).abs() <= 4_800, "{frames}");
    }

    #[test]
    fn video_without_audio_has_no_audio_decoder() {
        let path = temp_path("dec-noaudio", "a.mp4");
        make_sample(&path, 1, 320, 240, false);
        assert!(AudioDecoder::open(&path).unwrap().is_none());
        assert!(!VideoDecoder::open(&path).unwrap().1.has_audio);
    }

    #[test]
    fn vietnamese_file_names_open() {
        let path = temp_path("dec-vi", "bài giảng.mp4");
        make_sample(&path, 1, 320, 240, false);
        assert!(VideoDecoder::open(&path).is_ok());
    }
}
