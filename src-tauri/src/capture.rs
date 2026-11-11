//! Screen capture sources on top of Windows Graphics Capture (via `windows-capture`).
//!
//! Three targets: a monitor, one window (its own content, even when covered), and a region of
//! a monitor (the monitor is captured and cropped).
use hp_core::geometry::{Rect, clamp_in_monitor, crop_in_monitor};
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use std::time::Duration;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, HMONITOR, MONITORINFO};
use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame as WgcFrame;
use windows_capture::graphics_capture_api::{GraphicsCaptureApi, InternalCaptureControl};
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, GraphicsCaptureItemType,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

/// What to capture. `Region::rect` is in virtual-screen pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Target {
    Monitor { index: usize },
    Window { hwnd: isize },
    Region { monitor: usize, rect: Rect },
}

impl Target {
    pub fn mode_name(&self) -> &'static str {
        match self {
            Target::Monitor { .. } => "monitor",
            Target::Window { .. } => "window",
            Target::Region { .. } => "region",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    pub rect: Rect,
    pub primary: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub process: String,
    pub rect: Rect,
}

/// One captured frame, top-down BGRA. `time_100ns` is the system-relative (QPC) capture time
/// in 100 ns units — the same clock `clock::now_100ns` reads.
pub struct Frame<'a> {
    pub bgra: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub time_100ns: i64,
}

pub trait FrameSink: Send {
    fn on_frame(&mut self, frame: Frame<'_>);
    /// The captured window was closed.
    fn on_closed(&mut self);
}

/// Which part of each captured frame reaches the sink.
#[derive(Clone, Copy, Debug)]
enum Crop {
    /// Whole item, trimmed to even width and height (video).
    Even,
    /// Whole item, every pixel (screenshots).
    Exact,
    /// Fixed rectangle relative to the item's top-left corner.
    Fixed(Rect),
}

struct Handler {
    sink: Box<dyn FrameSink>,
    crop: Crop,
}

impl GraphicsCaptureApiHandler for Handler {
    type Flags = (Box<dyn FrameSink>, Crop);
    type Error = String;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let (sink, crop) = ctx.flags;
        Ok(Self { sink, crop })
    }

    fn on_frame_arrived(&mut self, frame: &mut WgcFrame, _control: InternalCaptureControl) -> Result<(), Self::Error> {
        let time_100ns = frame.timestamp().map(|t| t.Duration).unwrap_or(0);
        let (frame_w, frame_h) = (frame.width(), frame.height());
        let (x0, y0, x1, y1) = match self.crop {
            Crop::Even => (0, 0, frame_w & !1, frame_h & !1),
            Crop::Exact => (0, 0, frame_w, frame_h),
            Crop::Fixed(r) => {
                let x0 = (r.x.max(0) as u32).min(frame_w);
                let y0 = (r.y.max(0) as u32).min(frame_h);
                (x0, y0, (x0 + r.w).min(frame_w), (y0 + r.h).min(frame_h))
            }
        };
        if x1 <= x0 || y1 <= y0 {
            return Ok(()); // window shrunk to nothing; skip the frame
        }
        let mut buffer = frame.buffer_crop(x0, y0, x1, y1).map_err(|e| e.to_string())?;
        let (width, height, stride) = (buffer.width(), buffer.height(), buffer.row_pitch());
        self.sink.on_frame(Frame { bgra: buffer.as_raw_buffer(), width, height, stride, time_100ns });
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.sink.on_closed();
        Ok(())
    }
}

/// A running capture. Dropping it stops the capture and waits for its thread.
pub struct CaptureSession {
    control: Option<CaptureControl<Handler, String>>,
}

impl Drop for CaptureSession {
    fn drop(&mut self) {
        if let Some(control) = self.control.take() {
            let _ = control.stop();
        }
    }
}

pub fn list_monitors() -> Vec<MonitorInfo> {
    let monitors = Monitor::enumerate().unwrap_or_default();
    monitors
        .iter()
        .enumerate()
        .filter_map(|(index, m)| {
            let (rect, primary) = monitor_rect(m)?;
            let name = m
                .name()
                .ok()
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| format!("Màn hình {}", index + 1));
            Some(MonitorInfo { index, name, rect, primary })
        })
        .collect()
}

pub fn list_windows() -> Vec<WindowInfo> {
    let own_pid = std::process::id();
    Window::enumerate()
        .unwrap_or_default()
        .into_iter()
        .filter(|w| w.process_id().map(|pid| pid != own_pid).unwrap_or(false))
        .filter_map(|w| {
            let title = w.title().ok().filter(|t| !t.trim().is_empty())?;
            let r = w.rect().ok()?;
            let rect = Rect { x: r.left, y: r.top, w: (r.right - r.left).max(0) as u32, h: (r.bottom - r.top).max(0) as u32 };
            if rect.w == 0 || rect.h == 0 {
                return None;
            }
            Some(WindowInfo {
                hwnd: w.as_raw_hwnd() as isize,
                title,
                process: w.process_name().unwrap_or_default(),
                rect,
            })
        })
        .collect()
}

/// Area of the virtual screen that `target` covers, for the input log header.
pub fn target_rect(target: &Target) -> Result<Rect, String> {
    match target {
        Target::Monitor { index } => Ok(monitor_at(*index)?.1),
        Target::Region { monitor, rect } => {
            let m = monitor_at(*monitor)?.1;
            let c = crop_in_monitor(*rect, m).ok_or("Vùng chọn quá nhỏ hoặc nằm ngoài màn hình")?;
            Ok(Rect { x: m.x + c.x, y: m.y + c.y, ..c })
        }
        Target::Window { hwnd } => {
            let r = Window::from_raw_hwnd(*hwnd as *mut _).rect().map_err(|e| e.to_string())?;
            Ok(Rect { x: r.left, y: r.top, w: (r.right - r.left).max(0) as u32, h: (r.bottom - r.top).max(0) as u32 })
        }
    }
}

/// Starts capturing video frames: even dimensions, cursor included, no yellow border.
pub fn start_capture(target: &Target, sink: Box<dyn FrameSink>) -> Result<CaptureSession, String> {
    start(target, sink, true)
}

/// Captures a single frame at full physical resolution as RGBA, without the cursor.
pub fn grab_frame(target: &Target) -> Result<image::RgbaImage, String> {
    struct OneShot(Option<mpsc::Sender<image::RgbaImage>>);
    impl FrameSink for OneShot {
        fn on_frame(&mut self, f: Frame<'_>) {
            let Some(tx) = self.0.take() else { return };
            let row_bytes = f.width as usize * 4;
            let mut rgba = Vec::with_capacity(row_bytes * f.height as usize);
            for row in f.bgra.chunks(f.stride as usize).take(f.height as usize) {
                for px in row[..row_bytes].as_chunks::<4>().0 {
                    rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
                }
            }
            if let Some(img) = image::RgbaImage::from_raw(f.width, f.height, rgba) {
                let _ = tx.send(img);
            }
        }
        fn on_closed(&mut self) {}
    }

    let (tx, rx) = mpsc::channel();
    let session = start(target, Box::new(OneShot(Some(tx))), false)?;
    let image = rx.recv_timeout(Duration::from_secs(3)).map_err(|_| "Không nhận được khung hình nào để chụp".to_string());
    drop(session);
    image
}

fn start(target: &Target, sink: Box<dyn FrameSink>, video: bool) -> Result<CaptureSession, String> {
    let whole = if video { Crop::Even } else { Crop::Exact };
    match target {
        Target::Monitor { index } => start_item(monitor_at(*index)?.0, sink, whole, video, false),
        Target::Region { monitor, rect } => {
            let (item, monitor_rect) = monitor_at(*monitor)?;
            let crop = if video { crop_in_monitor(*rect, monitor_rect) } else { clamp_in_monitor(*rect, monitor_rect) }
                .ok_or("Vùng chọn quá nhỏ hoặc nằm ngoài màn hình")?;
            start_item(item, sink, Crop::Fixed(crop), video, false)
        }
        Target::Window { hwnd } => {
            let window = Window::from_raw_hwnd(*hwnd as *mut _);
            if !window.is_valid() {
                return Err("Cửa sổ đã chọn không còn tồn tại hoặc không quay được".to_string());
            }
            start_item(window, sink, whole, video, true)
        }
    }
}

fn start_item<T>(item: T, sink: Box<dyn FrameSink>, crop: Crop, cursor: bool, is_window: bool) -> Result<CaptureSession, String>
where
    T: TryInto<GraphicsCaptureItemType> + Send + 'static,
{
    let border = if GraphicsCaptureApi::is_border_settings_supported().unwrap_or(false) {
        DrawBorderSettings::WithoutBorder
    } else {
        DrawBorderSettings::Default
    };
    // Menus and popups are separate windows; ask Windows to include them when it can.
    let secondary = if is_window && GraphicsCaptureApi::is_secondary_windows_supported().unwrap_or(false) {
        SecondaryWindowSettings::Include
    } else {
        SecondaryWindowSettings::Default
    };
    let settings = Settings::new(
        item,
        if cursor { CursorCaptureSettings::WithCursor } else { CursorCaptureSettings::WithoutCursor },
        border,
        secondary,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        (sink, crop),
    );
    let control = Handler::start_free_threaded(settings).map_err(|e| format!("Không bắt đầu ghi hình được: {e}"))?;
    Ok(CaptureSession { control: Some(control) })
}

fn monitor_at(index: usize) -> Result<(Monitor, Rect), String> {
    let monitor = Monitor::enumerate()
        .map_err(|e| e.to_string())?
        .into_iter()
        .nth(index)
        .ok_or("Không tìm thấy màn hình đã chọn")?;
    let rect = monitor_rect(&monitor).ok_or("Không đọc được kích thước màn hình")?.0;
    Ok((monitor, rect))
}

/// Monitor rectangle in virtual-screen pixels and whether it is the primary monitor.
fn monitor_rect(monitor: &Monitor) -> Option<(Rect, bool)> {
    const MONITORINFOF_PRIMARY: u32 = 1;
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: `info` is a valid MONITORINFO with cbSize set; the handle comes from enumeration.
    let ok = unsafe { GetMonitorInfoW(HMONITOR(monitor.as_raw_hmonitor()), &mut info) }.as_bool();
    if !ok {
        return None;
    }
    let r = info.rcMonitor;
    let rect = Rect { x: r.left, y: r.top, w: (r.right - r.left).max(0) as u32, h: (r.bottom - r.top).max(0) as u32 };
    Some((rect, info.dwFlags & MONITORINFOF_PRIMARY != 0))
}

#[cfg(test)]
mod desktop_tests {
    //! These need a real desktop session, so they are ignored by default:
    //! `cargo test -p hp-screen --lib -- --ignored --nocapture`
    use super::*;

    #[test]
    #[ignore = "needs a desktop session"]
    fn grab_primary_monitor_at_physical_resolution() {
        let monitors = list_monitors();
        println!("monitors: {monitors:?}");
        let primary = monitors.iter().find(|m| m.primary).expect("a primary monitor");
        let image = grab_frame(&Target::Monitor { index: primary.index }).expect("a frame");
        println!("grabbed {}x{}", image.width(), image.height());
        let lit = image.pixels().filter(|p| p.0[0] as u32 + p.0[1] as u32 + p.0[2] as u32 > 30).count();
        assert!(lit > 1000, "screenshot is almost entirely black");
        assert!(image.width() >= 800 && image.height() >= 600);
    }

    #[test]
    #[ignore = "needs a desktop session"]
    fn grab_odd_sized_region_keeps_every_pixel() {
        let primary = list_monitors().into_iter().find(|m| m.primary).expect("a primary monitor");
        let rect = Rect { x: primary.rect.x + 101, y: primary.rect.y + 51, w: 301, h: 201 };
        let image = grab_frame(&Target::Region { monitor: primary.index, rect }).expect("a frame");
        assert_eq!((image.width(), image.height()), (301, 201));
    }

    #[test]
    #[ignore = "needs a desktop session"]
    fn windows_are_listed_with_titles() {
        let windows = list_windows();
        println!("{} windows, first: {:?}", windows.len(), windows.first());
        assert!(!windows.is_empty());
        assert!(windows.iter().all(|w| !w.title.trim().is_empty()));
    }
}
