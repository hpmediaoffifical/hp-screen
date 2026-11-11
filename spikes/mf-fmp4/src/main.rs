//! THROWAWAY SPIKE — not part of the app.
//!
//!   mf-fmp4-spike write <file> <seconds> <frag|plain>   write a test video in real time
//!   mf-fmp4-spike probe <file>                           count the frames that can be decoded
use std::io::Write;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::core::{GUID, HSTRING, Result};

const W: u32 = 1280;
const H: u32 = 720;
const FPS: u32 = 30;
const MF_VERSION: u32 = 0x0002_0070;

fn pack(hi: u32, lo: u32) -> u64 {
    ((hi as u64) << 32) | lo as u64
}

unsafe fn video_type(subtype: &GUID) -> Result<IMFMediaType> {
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        t.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        t.SetUINT64(&MF_MT_FRAME_SIZE, pack(W, H))?;
        t.SetUINT64(&MF_MT_FRAME_RATE, pack(FPS, 1))?;
        t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
        Ok(t)
    }
}

unsafe fn write(path: &str, seconds: u32, fragmented: bool) -> Result<()> {
    unsafe {
        let mut attrs = None;
        MFCreateAttributes(&mut attrs, 4)?;
        let attrs = attrs.unwrap();
        attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
        if fragmented {
            attrs.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_FMPEG4)?;
        }
        let writer = MFCreateSinkWriterFromURL(&HSTRING::from(path), None, &attrs)?;

        let out = video_type(&MFVideoFormat_H264)?;
        out.SetUINT32(&MF_MT_AVG_BITRATE, 4_000_000)?;
        let stream = writer.AddStream(&out)?;

        let input = video_type(&MFVideoFormat_RGB32)?;
        input.SetUINT32(&MF_MT_DEFAULT_STRIDE, W * 4)?;
        writer.SetInputMediaType(stream, &input, None)?;
        writer.BeginWriting()?;

        let frame_bytes = (W * H * 4) as usize;
        let duration = 10_000_000i64 / FPS as i64;
        let start = std::time::Instant::now();
        for n in 0..seconds * FPS {
            let buffer = MFCreateMemoryBuffer(frame_bytes as u32)?;
            let mut data = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None)?;
            let pixels = std::slice::from_raw_parts_mut(data, frame_bytes);
            // A colour that changes every frame, plus a moving bar, so frames are distinct.
            let shade = (n * 4 % 256) as u8;
            let bar = (n * 8 % W) as usize;
            for (i, px) in pixels.chunks_exact_mut(4).enumerate() {
                let x = i % W as usize;
                let on_bar = x >= bar && x < bar + 40;
                px.copy_from_slice(&[shade, if on_bar { 255 } else { 60 }, 255 - shade, 255]);
            }
            buffer.Unlock()?;
            buffer.SetCurrentLength(frame_bytes as u32)?;

            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(n as i64 * duration)?;
            sample.SetSampleDuration(duration)?;
            writer.WriteSample(stream, &sample)?;

            if n % FPS == 0 {
                println!("writing second {}", n / FPS);
                std::io::stdout().flush().ok();
            }
            // Real-time pacing so killing the process at second N means N seconds were written.
            let due = std::time::Duration::from_micros((n as u64 + 1) * 1_000_000 / FPS as u64);
            if let Some(wait) = due.checked_sub(start.elapsed()) {
                std::thread::sleep(wait);
            }
        }
        writer.Finalize()?;
        println!("finalized");
        Ok(())
    }
}

unsafe fn probe(path: &str) -> Result<()> {
    unsafe {
        let reader = MFCreateSourceReaderFromURL(&HSTRING::from(path), None)?;
        let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        let mut frames = 0u32;
        let mut last_time = 0i64;
        loop {
            let mut flags = 0u32;
            let mut time = 0i64;
            let mut sample = None;
            reader.ReadSample(stream, 0, None, Some(&mut flags), Some(&mut time), Some(&mut sample))?;
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                break;
            }
            if sample.is_some() {
                frames += 1;
                last_time = time;
            }
        }
        println!("frames={frames} last_time_s={:.2}", last_time as f64 / 10_000_000.0);
        Ok(())
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok().and_then(|_| MFStartup(MF_VERSION, MFSTARTUP_FULL))
            .and_then(|_| match args.get(1).map(String::as_str) {
                Some("write") => write(&args[2], args[3].parse().unwrap(), args[4] == "frag"),
                Some("probe") => probe(&args[2]),
                _ => {
                    eprintln!("usage: write <file> <seconds> <frag|plain> | probe <file>");
                    Ok(())
                }
            })
    };
    if let Err(e) = result {
        println!("ERROR {e}");
        std::process::exit(1);
    }
}
