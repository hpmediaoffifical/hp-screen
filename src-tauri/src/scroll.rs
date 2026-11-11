//! Scrolling capture: long pages that do not fit on screen. The user scrolls the page inside
//! the chosen area with their own mouse; HP Screen captures the area over and over and joins
//! the frames where they overlap (`hp_core::snip::Stitcher`).
//!
//! HP Screen never scrolls for the user: it sends no input of any kind. While capturing, a
//! click-through frame marks the area and a small bar shows progress; both stay out of the
//! captures like every other HP Screen window.
use crate::capture::{Target, grab_frame};
use crate::commands::AppState;
use crate::{events, screenshot, shell, snip};
use hp_core::geometry::{MIN_SIZE, Rect};
use hp_core::snip::{StitchStep, Stitcher};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder, WindowEvent};

pub const BAR: &str = "scroll-bar";
pub const FRAME: &str = "scroll-frame";
/// Time between two captures of the area.
const INTERVAL: Duration = Duration::from_millis(110);
/// Memory the joined picture may use (RGBA), and its tallest height in any case.
const MAX_BYTES: usize = 160_000_000;
const MAX_ROWS: usize = 30_000;
/// Thickness of the marker frame drawn just outside the area, physical pixels.
const FRAME_OUTSET: i32 = 4;

struct Session {
    stop: Arc<AtomicBool>,
    worker: JoinHandle<Stitcher>,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);
/// A finished long picture not saved yet (saving failed): Done tries again with it.
static UNSAVED: Mutex<Option<image::RgbaImage>> = Mutex::new(None);

/// Progress for the bar, sent after every capture.
#[derive(Clone, Debug, Serialize)]
pub struct ScrollProgress {
    pub width: usize,
    pub height: usize,
    /// "started", "added", "same", "lost" (frame did not continue the page) or "full".
    pub status: &'static str,
}

/// Starts capturing `rect` (monitor-relative physical pixels of the overlay's monitor).
pub fn start(app: &AppHandle, rect: Rect) -> Result<(), String> {
    let ctx = snip::context(app).ok_or("Công cụ chụp ảnh chưa được mở")?;
    if rect.w < MIN_SIZE || rect.h < MIN_SIZE {
        return Err(format!("Vùng chụp cuộn cần rộng và cao ít nhất {MIN_SIZE} px."));
    }
    // Not held while windows are built: the main thread may need it meanwhile (see `stop`).
    if SESSION.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
        return Ok(());
    }
    let monitor = ctx.monitor;
    let region = Rect { x: monitor.rect.x + rect.x, y: monitor.rect.y + rect.y, ..rect };

    // The frozen picture steps aside so the live page shows (and takes the mouse wheel).
    let overlay = app.get_webview_window(snip::SNIP);
    if let Some(overlay) = &overlay {
        let _ = overlay.hide();
    }
    if let Err(e) = open_windows(app, region, monitor.rect) {
        // Back to the overlay, so the user is not left with nothing on screen.
        for label in [FRAME, BAR] {
            if let Some(window) = app.get_webview_window(label) {
                let _ = window.destroy();
            }
        }
        if let Some(overlay) = &overlay {
            let _ = overlay.show();
            let _ = overlay.set_focus();
        }
        return Err(e);
    }

    let stop_flag = Arc::new(AtomicBool::new(false));
    let worker = {
        let stop_flag = stop_flag.clone();
        let app = app.clone();
        let target = Target::Region { monitor: monitor.index, rect: region };
        let (w, h) = (region.w as usize, region.h as usize);
        let max_rows = (MAX_BYTES / (w * 4).max(1)).clamp(h, MAX_ROWS);
        std::thread::spawn(move || {
            let mut stitcher = Stitcher::new(w, h, max_rows);
            while !stop_flag.load(Ordering::SeqCst) {
                if let Ok(picture) = grab_frame(&target)
                    && picture.dimensions() == (w as u32, h as u32)
                {
                    let status = match stitcher.push(picture.as_raw()) {
                        StitchStep::Started => "started",
                        StitchStep::Added(_) => "added",
                        StitchStep::Unchanged => "same",
                        StitchStep::NoMatch => "lost",
                        StitchStep::Full => "full",
                    };
                    let _ = app.emit("scroll-progress", ScrollProgress { width: w, height: stitcher.height(), status });
                }
                std::thread::sleep(INTERVAL);
            }
            stitcher
        })
    };
    *SESSION.lock().unwrap_or_else(|e| e.into_inner()) = Some(Session { stop: stop_flag, worker });
    Ok(())
}

/// The click-through frame around the area and the progress bar beside it.
fn open_windows(app: &AppHandle, region: Rect, monitor: Rect) -> Result<(), String> {
    let frame =WebviewWindowBuilder::new(app, FRAME, WebviewUrl::App("index.html".into()))
        .title("HP Screen - Vùng chụp cuộn")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .build()
        .map_err(|e| format!("Không mở được khung chụp cuộn: {e}"))?;
    shell::exclude_from_capture(&frame);
    let _ = frame.set_ignore_cursor_events(true);
    let _ = frame.set_position(PhysicalPosition::new(region.x - FRAME_OUTSET, region.y - FRAME_OUTSET));
    let _ = frame.set_size(PhysicalSize::new(region.w + 2 * FRAME_OUTSET as u32, region.h + 2 * FRAME_OUTSET as u32));
    let _ = frame.show();

    let bar = WebviewWindowBuilder::new(app, BAR, WebviewUrl::App("index.html".into()))
        .title("HP Screen - Chụp cuộn")
        .decorations(false)
        .shadow(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .inner_size(560.0, 58.0)
        .visible(false)
        .build()
        .map_err(|e| format!("Không mở được thanh chụp cuộn: {e}"))?;
    shell::exclude_from_capture(&bar);
    // Moved onto the area's monitor first: its scale decides the bar's real size there.
    let _ = bar.set_position(PhysicalPosition::new(region.x, region.y));
    if let Ok(size) = bar.outer_size() {
        let (x, y) = bar_position(region, monitor, size.width, size.height);
        let _ = bar.set_position(PhysicalPosition::new(x, y));
    }
    let _ = bar.show();
    // Closed some other way (Alt+F4): stop and throw the capture away.
    let app = app.clone();
    bar.on_window_event(move |event| {
        if let WindowEvent::Destroyed = event {
            let _ = stop(&app, false);
        }
    });
    Ok(())
}

/// Bar under the area, else above it, else inside its bottom edge; centred on it, on screen.
fn bar_position(region: Rect, monitor: Rect, bar_w: u32, bar_h: u32) -> (i32, i32) {
    let gap = 10;
    let (bar_w, bar_h) = (bar_w as i32, bar_h as i32);
    let below = region.y + region.h as i32 + FRAME_OUTSET + gap;
    let above = region.y - FRAME_OUTSET - gap - bar_h;
    let bottom_limit = monitor.y + monitor.h as i32;
    let y = if below + bar_h <= bottom_limit {
        below
    } else if above >= monitor.y {
        above
    } else {
        region.y + region.h as i32 - bar_h - gap
    };
    let x = region.x + (region.w as i32 - bar_w) / 2;
    let x = x.clamp(monitor.x, (monitor.x + monitor.w as i32 - bar_w).max(monitor.x));
    (x, y)
}

/// Ends the capture. With `keep`, the joined picture is saved and copied and its path
/// returned; either way the frame and the capture-and-annotate overlay close. The bar stays
/// to show the result and closes itself (`close_bar`).
pub fn stop(app: &AppHandle, keep: bool) -> Result<Option<(String, u32, u32)>, String> {
    if let Some(session) = SESSION.lock().unwrap_or_else(|e| e.into_inner()).take() {
        session.stop.store(true, Ordering::SeqCst);
        if let Some(frame) = app.get_webview_window(FRAME) {
            let _ = frame.destroy();
        }
        snip::close(app);
        if !keep {
            // The worker notices the flag and ends by itself; no need to wait for it.
            return Ok(None);
        }
        let stitcher = session.worker.join().map_err(|_| "Chụp cuộn bị lỗi".to_string())?;
        let (width, height, rgba) = stitcher.finish();
        let picture = image::RgbaImage::from_raw(width as u32, height as u32, rgba)
            .filter(|p| p.height() > 0)
            .ok_or("Chưa chụp được gì. Hãy thử lại.")?;
        *UNSAVED.lock().unwrap_or_else(|e| e.into_inner()) = Some(picture);
    }
    let mut unsaved = UNSAVED.lock().unwrap_or_else(|e| e.into_inner());
    if !keep {
        unsaved.take();
        return Ok(None);
    }
    let Some(picture) = unsaved.as_ref() else { return Ok(None) };
    // Kept until it is saved, so a failed save (folder missing, disk full) can be retried.
    let settings = app.state::<AppState>().settings();
    let dir = crate::paths::output_dir(app, &settings)?;
    let path = screenshot::save_png(&dir, &settings, picture)?;
    events::library_changed(app);
    // Saved already; a busy clipboard must not turn the capture into an error.
    let _ = screenshot::copy_to_clipboard(picture);
    let size = picture.dimensions();
    unsaved.take();
    Ok(Some((path.to_string_lossy().into_owned(), size.0, size.1)))
}

pub fn close_bar(app: &AppHandle) {
    UNSAVED.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(bar) = app.get_webview_window(BAR) {
        let _ = bar.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect { x: 0, y: 0, w: 1920, h: 1080 };

    #[test]
    fn the_bar_sits_under_the_area_when_there_is_room() {
        let region = Rect { x: 600, y: 100, w: 700, h: 600 };
        assert_eq!(bar_position(region, SCREEN, 500, 60), (700, 714));
    }

    #[test]
    fn the_bar_moves_above_or_inside_the_area_near_the_bottom() {
        let low = Rect { x: 600, y: 500, w: 700, h: 560 };
        assert_eq!(bar_position(low, SCREEN, 500, 60), (700, 426));
        let tall = Rect { x: 0, y: 20, w: 1920, h: 1060 };
        assert_eq!(bar_position(tall, SCREEN, 500, 60), (710, 1010));
    }

    #[test]
    fn the_bar_stays_on_its_monitor() {
        let left = Rect { x: -1920, y: 0, w: 1920, h: 1080 };
        let region = Rect { x: -1910, y: 100, w: 200, h: 300 };
        assert_eq!(bar_position(region, left, 500, 60).0, -1920);
    }
}

#[cfg(test)]
mod desktop_tests {
    //! Captures the real screen. Needs a page framed in magenta (#ff00ff) that scrolls by
    //! itself while the test runs; no input is sent. Run on request:
    //! `HP_SCROLL_OUT=out.png cargo test -p hp-screen --lib -- --ignored live_scroll --nocapture`
    use super::*;
    use crate::capture::list_monitors;
    use std::time::Instant;

    /// Bounding box of the magenta frame on a monitor picture, if there is one.
    fn magenta_box(picture: &image::RgbaImage) -> Option<(u32, u32, u32, u32)> {
        let (mut x0, mut y0, mut x1, mut y1, mut count) = (u32::MAX, u32::MAX, 0, 0, 0u32);
        for (x, y, p) in picture.enumerate_pixels() {
            if p[0] > 235 && p[1] < 25 && p[2] > 235 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                count += 1;
            }
        }
        (count > 2000).then_some((x0, y0, x1, y1))
    }

    #[test]
    #[ignore = "captures the real screen while a marked page scrolls"]
    fn live_scroll_capture_of_a_marked_area() {
        use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
        // Physical pixels, like the app itself.
        // SAFETY: plain process setting, called before any window or capture work.
        let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        let secs: u64 = std::env::var("HP_SCROLL_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(14);
        // The marked window may take a moment to appear.
        let waiting = Instant::now();
        let (monitor, (x0, y0, x1, y1)) = loop {
            let found = list_monitors().into_iter().find_map(|m| {
                let shot = grab_frame(&Target::Monitor { index: m.index }).ok()?;
                magenta_box(&shot).map(|b| (m, b))
            });
            if let Some(found) = found {
                break found;
            }
            assert!(waiting.elapsed() < Duration::from_secs(15), "no magenta frame on any monitor");
            std::thread::sleep(Duration::from_millis(300));
        };
        // Inside the frame (its border is a few pixels thick at any scale).
        let inset = 14;
        let region = Rect {
            x: monitor.rect.x + (x0 + inset) as i32,
            y: monitor.rect.y + (y0 + inset) as i32,
            w: x1 - x0 - 2 * inset,
            h: y1 - y0 - 2 * inset,
        };
        println!("monitor {} region {region:?}", monitor.index);
        let target = Target::Region { monitor: monitor.index, rect: region };
        let (w, h) = (region.w as usize, region.h as usize);
        let mut stitcher = Stitcher::new(w, h, MAX_ROWS);
        let (mut added, mut same, mut lost) = (0, 0, 0);
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(secs) {
            if let Ok(picture) = grab_frame(&target)
                && picture.dimensions() == (region.w, region.h)
            {
                match stitcher.push(picture.as_raw()) {
                    StitchStep::Added(_) => added += 1,
                    StitchStep::Unchanged => same += 1,
                    StitchStep::NoMatch => lost += 1,
                    _ => {}
                }
            }
            std::thread::sleep(INTERVAL);
        }
        let (width, height, rgba) = stitcher.finish();
        println!("added {added}, unchanged {same}, lost {lost}; picture {width} x {height}");
        let picture = image::RgbaImage::from_raw(width as u32, height as u32, rgba).expect("picture");
        if let Some(out) = std::env::var_os("HP_SCROLL_OUT") {
            picture.save(out).expect("save");
        }
        assert!(added >= 3, "the page never seemed to scroll");
        assert!(height > h * 2, "joined picture is not much taller than one frame");
    }
}
