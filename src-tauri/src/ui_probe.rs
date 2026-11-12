//! Boxes of UI elements for automatic zoom: the element under a click and the one receiving
//! keys. Read with UI Automation on a thread of its own, so the input hooks never wait on
//! another application. Only boxes are read: never names, text or types.
use hp_core::geometry::Rect;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// What to look up, stamped with video time.
pub enum Ask {
    /// The element under this screen point (a left click).
    Point { t: i64, x: i32, y: i32, asked: Instant },
    /// The element receiving keys (typing).
    Focus { t: i64, asked: Instant },
}

/// A question older than this when its turn comes is dropped: the screen may have changed.
const STALE: Duration = Duration::from_millis(300);
/// UI Automation gives up on an application that does not answer within this time.
const UIA_TIMEOUT_MS: u32 = 500;

/// A box found: video time, "click" or "focus", screen rectangle (left, top, right, bottom).
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub t: i64,
    pub kind: &'static str,
    pub ltrb: [i32; 4],
}

/// Running UI Automation thread.
pub struct UiProbe {
    tx: Option<Sender<Ask>>,
    rx: Receiver<Found>,
    thread: Option<JoinHandle<()>>,
}

/// Elements smaller than this (px) are parts of a control (an icon, a label): their parent is used.
const MIN_W: i32 = 16;
const MIN_H: i32 = 12;
const MAX_PARENT_STEPS: usize = 4;

/// Whether the top-level window at screen point (x, y) is HP Screen's own (the control bar):
/// clicks there are never zoom targets. Checked by window, since the bar's content runs in
/// WebView2's own process.
pub fn own_window_at(x: i32, y: i32) -> bool {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{GA_ROOT, GetAncestor, GetWindowThreadProcessId, WindowFromPoint};
    // SAFETY: plain window queries with valid arguments; a missing window gives a null handle.
    unsafe {
        let hwnd = WindowFromPoint(POINT { x, y });
        if hwnd.is_invalid() {
            return false;
        }
        let root = GetAncestor(hwnd, GA_ROOT);
        let mut pid = 0u32;
        GetWindowThreadProcessId(if root.is_invalid() { hwnd } else { root }, Some(&mut pid));
        pid == std::process::id()
    }
}

/// `ltrb` relative to the captured `region`, clipped to it, as `[x, y, w, h]`; `None` outside it.
pub fn rel_rect(ltrb: [i32; 4], region: Rect) -> Option<[i32; 4]> {
    let (rl, rt) = (region.x, region.y);
    let (rr, rb) = (region.x + region.w as i32, region.y + region.h as i32);
    let (l, t, r, b) = (ltrb[0].max(rl), ltrb[1].max(rt), ltrb[2].min(rr), ltrb[3].min(rb));
    (r > l && b > t).then(|| [l - rl, t - rt, r - l, b - t])
}

impl UiProbe {
    pub fn start() -> UiProbe {
        let (tx, ask_rx) = mpsc::channel::<Ask>();
        let (found_tx, rx) = mpsc::channel::<Found>();
        let thread = std::thread::Builder::new().name("hp-ui-probe".into()).spawn(move || run(ask_rx, found_tx)).ok();
        UiProbe { tx: Some(tx), rx, thread }
    }

    pub fn ask(&self, a: Ask) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(a);
        }
    }

    /// Boxes found so far.
    pub fn take(&self) -> Vec<Found> {
        self.rx.try_iter().collect()
    }

    /// Stops taking questions and returns the remaining answers, waiting at most `wait`
    /// (another application that hangs must not hang the end of a recording).
    pub fn finish(mut self, wait: Duration) -> Vec<Found> {
        self.tx = None;
        let deadline = Instant::now() + wait;
        let mut out = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(f) => out.push(f),
                Err(RecvTimeoutError::Disconnected) => {
                    if let Some(t) = self.thread.take() {
                        let _ = t.join();
                    }
                    break;
                }
                Err(RecvTimeoutError::Timeout) => break, // the thread is left to finish on its own
            }
        }
        out
    }
}

fn run(asks: Receiver<Ask>, found: Sender<Found>) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationElement};
    // SAFETY: COM is initialised on this thread before any UI Automation call; every call gets
    // valid arguments, and failures are skipped.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let Ok(uia) = CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER) else { return };
        // A hung application must not hold the questions behind it for long.
        if let Ok(uia2) = windows::core::Interface::cast::<windows::Win32::UI::Accessibility::IUIAutomation2>(&uia) {
            let _ = uia2.SetConnectionTimeout(UIA_TIMEOUT_MS);
            let _ = uia2.SetTransactionTimeout(UIA_TIMEOUT_MS);
        }
        let walker = uia.ControlViewWalker().ok();
        let own = std::process::id() as i32;
        let rect_of = |e: &IUIAutomationElement| e.CurrentBoundingRectangle().ok().map(|r| [r.left, r.top, r.right, r.bottom]);
        for ask in asks {
            let asked = match ask {
                Ask::Point { asked, .. } | Ask::Focus { asked, .. } => asked,
            };
            if asked.elapsed() > STALE {
                continue;
            }
            let (t, kind, element) = match ask {
                Ask::Point { t, x, y, .. } => (t, "click", uia.ElementFromPoint(POINT { x, y }).ok()),
                Ask::Focus { t, .. } => (t, "focus", uia.GetFocusedElement().ok()),
            };
            let Some(mut el) = element else { continue };
            // HP Screen's own windows (the control bar) are never zoom targets.
            if el.CurrentProcessId().ok() == Some(own) {
                continue;
            }
            let Some(mut r) = rect_of(&el) else { continue };
            for _ in 0..MAX_PARENT_STEPS {
                if r[2].saturating_sub(r[0]) >= MIN_W && r[3].saturating_sub(r[1]) >= MIN_H {
                    break;
                }
                let Some(parent) = walker.as_ref().and_then(|w| w.GetParentElement(&el).ok()) else { break };
                let Some(pr) = rect_of(&parent) else { break };
                (el, r) = (parent, pr);
            }
            if r[2] > r[0] && r[3] > r[1] && found.send(Found { t, kind, ltrb: r }).is_err() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_are_made_relative_and_clipped_to_the_region() {
        let region = Rect { x: 100, y: 50, w: 400, h: 300 };
        assert_eq!(rel_rect([150, 60, 250, 90], region), Some([50, 10, 100, 30]));
        assert_eq!(rel_rect([0, 0, 200, 100], region), Some([0, 0, 100, 50]), "clipped");
        assert_eq!(rel_rect([600, 0, 700, 40], region), None, "outside");
    }

    #[test]
    fn the_probe_starts_and_finishes_without_questions() {
        let p = UiProbe::start();
        let t = Instant::now();
        assert!(p.finish(Duration::from_secs(2)).is_empty());
        assert!(t.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn questions_that_waited_too_long_are_dropped() {
        // A stalled application must not get later clicks matched against a changed screen.
        let p = UiProbe::start();
        let old = Instant::now() - Duration::from_secs(1);
        p.ask(Ask::Point { t: 1, x: 5, y: 5, asked: old });
        p.ask(Ask::Focus { t: 2, asked: old });
        assert!(p.finish(Duration::from_secs(2)).is_empty());
    }

    /// Needs a desktop session: `cargo test -p hp-screen --lib ui_probe -- --ignored --nocapture`
    #[test]
    #[ignore = "needs a desktop session"]
    fn the_focused_element_has_a_box() {
        let p = UiProbe::start();
        p.ask(Ask::Focus { t: 1, asked: Instant::now() });
        p.ask(Ask::Point { t: 2, x: 5, y: 5, asked: Instant::now() });
        let found = p.finish(Duration::from_secs(3));
        println!("{found:?}");
        assert!(!found.is_empty());
    }
}
