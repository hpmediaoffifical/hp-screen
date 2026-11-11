//! Webcams through Media Foundation: list them, open one, and keep its latest frame as BGRA
//! for the live layer. Reading runs on its own thread; dropping the camera releases it.
use crate::clock;
use crate::decoder::copy_bgra;
use crate::encoder::init_thread;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use windows::Win32::Foundation::E_ACCESSDENIED;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::core::{HRESULT, PWSTR};

const VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
/// Highest camera resolution read: the bubble is small, 720p is plenty.
const MAX_HEIGHT: u32 = 720;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CameraInfo {
    /// Symbolic link of the device.
    pub id: String,
    pub name: String,
}

/// One camera picture, top-down BGRA.
#[derive(Clone, Debug)]
pub struct CamFrame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
    /// When it arrived, on the recording clock (ms).
    pub at_ms: i64,
}

pub type SharedFrame = Arc<Mutex<Option<CamFrame>>>;

/// The video capture devices, each as an activation object with its id and name.
unsafe fn devices() -> windows::core::Result<Vec<(IMFActivate, CameraInfo)>> {
    unsafe {
        let mut attrs = None;
        MFCreateAttributes(&mut attrs, 1)?;
        let attrs = attrs.ok_or_else(windows::core::Error::empty)?;
        attrs.SetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID)?;
        let (mut list, mut count) = (std::ptr::null_mut(), 0u32);
        MFEnumDeviceSources(&attrs, &mut list, &mut count)?;
        let mut out = Vec::new();
        if !list.is_null() {
            for i in 0..count as usize {
                // Take ownership of each entry so it is released.
                let Some(activate) = std::ptr::read(list.add(i)) else { continue };
                let id = string_attr(&activate, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK).unwrap_or_default();
                let name = string_attr(&activate, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME).unwrap_or_else(|| format!("Camera {}", i + 1));
                out.push((activate, CameraInfo { id, name }));
            }
            CoTaskMemFree(Some(list as *const _));
        }
        Ok(out)
    }
}

unsafe fn string_attr(a: &IMFActivate, key: &windows::core::GUID) -> Option<String> {
    unsafe {
        let (mut value, mut len) = (PWSTR::null(), 0u32);
        a.GetAllocatedString(key, &mut value, &mut len).ok()?;
        let text = value.to_string().ok();
        CoTaskMemFree(Some(value.0 as *const _));
        text
    }
}

pub fn list_cameras() -> Vec<CameraInfo> {
    init_thread();
    // SAFETY: Media Foundation is started on this thread.
    unsafe { devices() }.map(|d| d.into_iter().map(|(_, info)| info).collect()).unwrap_or_default()
}

pub struct Camera {
    latest: SharedFrame,
    alive: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Camera {
    /// Opens camera `id` (the first one when `None`) and starts reading frames.
    pub fn open(id: Option<&str>) -> Result<Camera, String> {
        let latest: SharedFrame = Arc::new(Mutex::new(None));
        let alive = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let id = id.map(str::to_string);
        let thread = {
            let (latest, alive, stop) = (latest.clone(), alive.clone(), stop.clone());
            std::thread::Builder::new()
                .name("hp-camera".into())
                .spawn(move || {
                    init_thread();
                    // SAFETY: all Media Foundation objects are created and used on this thread.
                    unsafe { read_camera(id.as_deref(), &latest, &stop, &ready_tx) };
                    alive.store(false, Ordering::SeqCst);
                })
                .map_err(|e| format!("Không tạo được luồng camera: {e}"))?
        };
        let opened = ready_rx.recv().unwrap_or_else(|_| Err("Không mở được camera".to_string()));
        let cam = Camera { latest, alive, stop, thread: Some(thread) };
        opened.map(|_| cam)
    }

    pub fn latest(&self) -> SharedFrame {
        self.latest.clone()
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Cleared when the reading thread stops (device unplugged or failed).
    pub fn alive_flag(&self) -> Arc<AtomicBool> {
        self.alive.clone()
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Opens the device, reports the outcome on `ready`, then keeps `latest` up to date until
/// `stop` is set or the device fails. The device is shut down (its light goes off) on return.
unsafe fn read_camera(id: Option<&str>, latest: &SharedFrame, stop: &AtomicBool, ready: &mpsc::Sender<Result<(), String>>) {
    unsafe {
        let opened = (|| -> Result<(IMFMediaSource, IMFSourceReader, u32, u32, i32), String> {
            let list = devices().map_err(|e| camera_error(e.code()))?;
            let chosen = match id {
                Some(id) => list.into_iter().find(|(_, info)| info.id == id),
                None => list.into_iter().next(),
            };
            let (activate, _) = chosen.ok_or("Không tìm thấy camera")?;
            let source: IMFMediaSource = activate.ActivateObject().map_err(|e| camera_error(e.code()))?;
            let mut attrs = None;
            MFCreateAttributes(&mut attrs, 1).map_err(|e| camera_error(e.code()))?;
            let attrs = attrs.ok_or("Không mở được camera")?;
            attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1).map_err(|e| camera_error(e.code()))?;
            let reader = MFCreateSourceReaderFromMediaSource(&source, &attrs).map_err(|e| camera_error(e.code()))?;
            let (w, h, rate) = best_native_size(&reader);
            let out = MFCreateMediaType().map_err(|e| camera_error(e.code()))?;
            out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|e| camera_error(e.code()))?;
            out.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).map_err(|e| camera_error(e.code()))?;
            if w > 0 && h > 0 {
                let _ = out.SetUINT64(&MF_MT_FRAME_SIZE, ((w as u64) << 32) | h as u64);
                if let Some(rate) = rate {
                    let _ = out.SetUINT64(&MF_MT_FRAME_RATE, rate);
                }
            }
            reader.SetCurrentMediaType(VIDEO, None, &out).map_err(|e| camera_error(e.code()))?;
            let current = reader.GetCurrentMediaType(VIDEO).map_err(|e| camera_error(e.code()))?;
            let size = current.GetUINT64(&MF_MT_FRAME_SIZE).map_err(|e| camera_error(e.code()))?;
            let (cw, ch) = ((size >> 32) as u32, size as u32);
            let stride = current.GetUINT32(&MF_MT_DEFAULT_STRIDE).map(|s| s as i32).unwrap_or(cw as i32 * 4);
            Ok((source, reader, cw, ch, stride))
        })();
        let (source, reader, w, h, stride) = match opened {
            Ok(v) => {
                let _ = ready.send(Ok(()));
                v
            }
            Err(e) => {
                let _ = ready.send(Err(e));
                return;
            }
        };
        while !stop.load(Ordering::SeqCst) {
            let (mut flags, mut sample) = (0u32, None);
            if reader.ReadSample(VIDEO, 0, None, Some(&mut flags), None, Some(&mut sample)).is_err() {
                break;
            }
            if flags & (MF_SOURCE_READERF_ERROR.0 as u32 | MF_SOURCE_READERF_ENDOFSTREAM.0 as u32) != 0 {
                break;
            }
            let Some(sample) = sample else { continue };
            let Ok(buffer) = sample.ConvertToContiguousBuffer() else { continue };
            if let Ok(bgra) = copy_bgra(&buffer, w, h, (0, 0), stride) {
                *latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(CamFrame { width: w, height: h, bgra, at_ms: clock::now_100ns() / 10_000 });
            }
        }
        let _ = source.Shutdown();
    }
}

/// The largest native picture no taller than `MAX_HEIGHT`, with its frame rate.
unsafe fn best_native_size(reader: &IMFSourceReader) -> (u32, u32, Option<u64>) {
    unsafe {
        let mut best = (0u32, 0u32, None);
        for i in 0.. {
            let Ok(t) = reader.GetNativeMediaType(VIDEO, i) else { break };
            let Ok(size) = t.GetUINT64(&MF_MT_FRAME_SIZE) else { continue };
            let (w, h) = ((size >> 32) as u32, size as u32);
            if h <= MAX_HEIGHT && w as u64 * h as u64 > best.0 as u64 * best.1 as u64 {
                best = (w, h, t.GetUINT64(&MF_MT_FRAME_RATE).ok());
            }
            if i > 200 {
                break;
            }
        }
        best
    }
}

/// A camera error as the user should read it.
fn camera_error(hr: HRESULT) -> String {
    match hr {
        h if h == E_ACCESSDENIED => {
            "Windows đang chặn quyền dùng camera cho ứng dụng desktop: mở Cài đặt › Quyền riêng tư và bảo mật › Camera".to_string()
        }
        h if h == MF_E_HW_MFT_FAILED_START_STREAMING || h == MF_E_VIDEO_RECORDING_DEVICE_PREEMPTED => "Camera đang được ứng dụng khác dùng".to_string(),
        h if h == MF_E_VIDEO_RECORDING_DEVICE_INVALIDATED => "Camera đã bị rút ra hoặc tắt".to_string(),
        h => format!("Không mở được camera (mã lỗi 0x{:08X})", h.0 as u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::E_ACCESSDENIED;
    use windows::Win32::Media::MediaFoundation::{MF_E_HW_MFT_FAILED_START_STREAMING, MF_E_VIDEO_RECORDING_DEVICE_INVALIDATED};

    #[test]
    fn listing_cameras_never_fails() {
        let cams = list_cameras();
        println!("{} camera(s): {cams:?}", cams.len());
    }

    #[test]
    fn camera_errors_are_explained() {
        assert!(camera_error(E_ACCESSDENIED).contains("Quyền riêng tư"));
        assert_eq!(camera_error(MF_E_HW_MFT_FAILED_START_STREAMING), "Camera đang được ứng dụng khác dùng");
        assert_eq!(camera_error(MF_E_VIDEO_RECORDING_DEVICE_INVALIDATED), "Camera đã bị rút ra hoặc tắt");
        assert!(camera_error(HRESULT(0x80004005u32 as i32)).starts_with("Không mở được camera"));
    }

    #[test]
    #[ignore = "needs a camera and the user's permission"]
    fn first_camera_gives_frames() {
        let cam = Camera::open(None).expect("a camera opens");
        let latest = cam.latest();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while latest.lock().unwrap().is_none() {
            assert!(std::time::Instant::now() < deadline, "no frame in 5 s");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let f = latest.lock().unwrap().clone().unwrap();
        println!("camera frame {}x{}", f.width, f.height);
        assert!(f.height <= 720 && f.bgra.len() == (f.width * f.height * 4) as usize);
        assert!(cam.is_alive());
    }
}
