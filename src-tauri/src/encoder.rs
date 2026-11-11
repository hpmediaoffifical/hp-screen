//! MP4 writer on Media Foundation: H.264 video (hardware encoder when available) and an
//! optional AAC track, in a fragmented MP4 so a crash does not lose the recording
//! (see spec 4.3 for the test behind that choice).
use std::path::PathBuf;
use std::sync::Once;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::core::{GUID, HSTRING};

pub const AUDIO_SAMPLE_RATE: u32 = 48_000;
pub const AUDIO_CHANNELS: u32 = 2;
const AAC_BYTES_PER_SEC: u32 = 24_000; // 192 kbps
const MF_VERSION: u32 = 0x0002_0070;
const H264_PROFILE_HIGH: u32 = 100;

#[derive(Clone, Debug)]
pub struct EncoderConfig {
    pub path: PathBuf,
    /// Even width and height in pixels.
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub video_bitrate: u32,
    pub audio: bool,
}

pub struct Mp4Encoder {
    writer: IMFSinkWriter,
    video_stream: u32,
    audio_stream: Option<u32>,
    width: u32,
    height: u32,
    frame_duration: i64,
}

// SAFETY: IMFSinkWriter is free-threaded (it lives in the MTA and serializes its own calls);
// every thread that touches it calls `init_thread` first.
unsafe impl Send for Mp4Encoder {}

/// Joins the calling thread to the COM multithreaded apartment and starts Media Foundation
/// once per process. Call on every thread that uses the encoder.
pub fn init_thread() {
    static START: Once = Once::new();
    // SAFETY: plain initialization calls; repeated CoInitializeEx on a thread is allowed.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        START.call_once(|| {
            let _ = MFStartup(MF_VERSION, MFSTARTUP_FULL);
        });
    }
}

fn pack(hi: u32, lo: u32) -> u64 {
    ((hi as u64) << 32) | lo as u64
}

fn err(context: &str, e: windows::core::Error) -> String {
    format!("{context}: {e}")
}

impl Mp4Encoder {
    /// Opens `cfg.path` for writing. Tries the hardware encoder first, then software.
    pub fn new(cfg: &EncoderConfig) -> Result<Self, String> {
        init_thread();
        if cfg.width == 0 || cfg.height == 0 || cfg.width % 2 != 0 || cfg.height % 2 != 0 {
            return Err(format!("Kích thước video không hợp lệ: {}x{}", cfg.width, cfg.height));
        }
        // SAFETY: Media Foundation is started; all pointers passed are valid for the calls.
        unsafe { Self::open(cfg, true).or_else(|_| Self::open(cfg, false)) }
            .map_err(|e| err("Không khởi tạo được bộ mã hóa video", e))
    }

    unsafe fn open(cfg: &EncoderConfig, hardware: bool) -> windows::core::Result<Self> {
        unsafe {
            let mut attrs = None;
            MFCreateAttributes(&mut attrs, 4)?;
            let attrs = attrs.ok_or_else(windows::core::Error::empty)?;
            attrs.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_FMPEG4)?;
            attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, hardware as u32)?;
            let writer = MFCreateSinkWriterFromURL(&HSTRING::from(cfg.path.as_os_str()), None, &attrs)?;

            let video_out = video_type(&MFVideoFormat_H264, cfg)?;
            video_out.SetUINT32(&MF_MT_AVG_BITRATE, cfg.video_bitrate)?;
            video_out.SetUINT32(&MF_MT_MPEG2_PROFILE, H264_PROFILE_HIGH)?;
            let video_stream = writer.AddStream(&video_out)?;
            let video_in = video_type(&MFVideoFormat_RGB32, cfg)?;
            // Positive stride: rows are stored top-down, as Windows Graphics Capture delivers them.
            video_in.SetUINT32(&MF_MT_DEFAULT_STRIDE, cfg.width * 4)?;
            writer.SetInputMediaType(video_stream, &video_in, None)?;

            let audio_stream = if cfg.audio {
                let audio_out = audio_type(&MFAudioFormat_AAC, 16)?;
                audio_out.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, AAC_BYTES_PER_SEC)?;
                let stream = writer.AddStream(&audio_out)?;
                let audio_in = audio_type(&MFAudioFormat_Float, 32)?;
                audio_in.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, AUDIO_CHANNELS * 4)?;
                audio_in.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, AUDIO_SAMPLE_RATE * AUDIO_CHANNELS * 4)?;
                writer.SetInputMediaType(stream, &audio_in, None)?;
                Some(stream)
            } else {
                None
            };

            writer.BeginWriting()?;
            Ok(Self {
                writer,
                video_stream,
                audio_stream,
                width: cfg.width,
                height: cfg.height,
                frame_duration: 10_000_000 / cfg.fps.max(1) as i64,
            })
        }
    }

    /// Writes one top-down BGRA frame of exactly the configured size.
    pub fn write_video(&mut self, bgra: &[u8], stride: u32, time_100ns: i64) -> Result<(), String> {
        let row = self.width as usize * 4;
        let stride = stride as usize;
        let rows = self.height as usize;
        if stride < row || bgra.len() < stride * (rows - 1) + row {
            return Err("Khung hình nhỏ hơn kích thước video".to_string());
        }
        // SAFETY: the locked buffer is `row * rows` bytes long and stays locked while written.
        unsafe {
            let buffer = MFCreateMemoryBuffer((row * rows) as u32).map_err(|e| err("Thiếu bộ nhớ cho khung hình", e))?;
            let mut data = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None).map_err(|e| err("Không khóa được bộ đệm", e))?;
            let out = std::slice::from_raw_parts_mut(data, row * rows);
            for (dst, src) in out.chunks_exact_mut(row).zip(bgra.chunks(stride)) {
                dst.copy_from_slice(&src[..row]);
            }
            let _ = buffer.Unlock();
            self.write_sample(self.video_stream, &buffer, (row * rows) as u32, time_100ns, self.frame_duration)
        }
    }

    /// Writes interleaved 48 kHz stereo float samples. Ignored when the file has no audio track.
    pub fn write_audio(&mut self, pcm: &[f32], time_100ns: i64) -> Result<(), String> {
        let Some(stream) = self.audio_stream else { return Ok(()) };
        if pcm.is_empty() {
            return Ok(());
        }
        let bytes = std::mem::size_of_val(pcm);
        let frames = pcm.len() as i64 / AUDIO_CHANNELS as i64;
        let duration = frames * 10_000_000 / AUDIO_SAMPLE_RATE as i64;
        // SAFETY: the locked buffer is `bytes` long; f32 samples are plain bytes.
        unsafe {
            let buffer = MFCreateMemoryBuffer(bytes as u32).map_err(|e| err("Thiếu bộ nhớ cho âm thanh", e))?;
            let mut data = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None).map_err(|e| err("Không khóa được bộ đệm", e))?;
            std::ptr::copy_nonoverlapping(pcm.as_ptr() as *const u8, data, bytes);
            let _ = buffer.Unlock();
            self.write_sample(stream, &buffer, bytes as u32, time_100ns, duration)
        }
    }

    unsafe fn write_sample(&self, stream: u32, buffer: &IMFMediaBuffer, len: u32, time: i64, duration: i64) -> Result<(), String> {
        unsafe {
            let write = || -> windows::core::Result<()> {
                buffer.SetCurrentLength(len)?;
                let sample = MFCreateSample()?;
                sample.AddBuffer(buffer)?;
                sample.SetSampleTime(time)?;
                sample.SetSampleDuration(duration)?;
                self.writer.WriteSample(stream, &sample)
            };
            write().map_err(|e| err("Ghi dữ liệu vào file video thất bại", e))
        }
    }

    /// Flushes the encoder and closes the file.
    pub fn finish(self) -> Result<(), String> {
        // SAFETY: the writer is valid and BeginWriting succeeded.
        unsafe { self.writer.Finalize() }.map_err(|e| err("Không đóng được file video", e))
    }
}

unsafe fn video_type(subtype: &GUID, cfg: &EncoderConfig) -> windows::core::Result<IMFMediaType> {
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        t.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        t.SetUINT64(&MF_MT_FRAME_SIZE, pack(cfg.width, cfg.height))?;
        t.SetUINT64(&MF_MT_FRAME_RATE, pack(cfg.fps, 1))?;
        t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
        Ok(t)
    }
}

unsafe fn audio_type(subtype: &GUID, bits: u32) -> windows::core::Result<IMFMediaType> {
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        t.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        t.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, AUDIO_SAMPLE_RATE)?;
        t.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, AUDIO_CHANNELS)?;
        t.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, bits)?;
        Ok(t)
    }
}

/// Reads a video back and reports (decodable video frames, time of the last one in 100 ns,
/// decodable audio samples). Used by the desktop tests to check what was actually written.
#[cfg(test)]
pub fn probe(path: &std::path::Path) -> Result<(u32, i64, u32), String> {
    init_thread();
    // SAFETY: Media Foundation is started; out pointers are valid.
    unsafe {
        let reader = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), None).map_err(|e| e.to_string())?;
        let mut count = |stream: u32| -> (u32, i64) {
            let (mut n, mut last) = (0u32, 0i64);
            loop {
                let (mut flags, mut time, mut sample) = (0u32, 0i64, None);
                if reader.ReadSample(stream, 0, None, Some(&mut flags), Some(&mut time), Some(&mut sample)).is_err() {
                    break;
                }
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    break;
                }
                if sample.is_some() {
                    n += 1;
                    last = time;
                }
            }
            (n, last)
        };
        let (video, last) = count(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32);
        let (audio, _) = count(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32);
        Ok((video, last, audio))
    }
}
