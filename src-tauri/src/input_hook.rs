//! Records mouse and shortcut activity next to a video (`<stem>.input.jsonl`) so later stages
//! can add click effects and automatic zoom. The hooks exist only while a recording runs.
//!
//! The hook callbacks do nothing but forward raw events; filtering (privacy, move rate) and
//! file writing happen on a separate thread so input is never delayed.
use crate::clock;
use hp_core::geometry::Rect;
use hp_core::input_log::{Button, Event, Header, KeyStroke, MoveSampler, loggable_combo, to_line};
use hp_core::timeline::Timeline;
use std::cell::RefCell;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, PostThreadMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
    WM_SYSKEYUP,
};

const FLUSH_INTERVAL: Duration = Duration::from_secs(1);

/// What a hook callback saw, stamped with the shared clock.
enum Raw {
    Mouse { time: i64, message: u32, x: i32, y: i32, wheel: i32 },
    KeyDown { time: i64, key: KeyStroke },
    KeyUp { vk: u32 },
}

thread_local! {
    /// Low-level hooks are called on the thread that installed them, so the sender lives here.
    static SENDER: RefCell<Option<Sender<Raw>>> = const { RefCell::new(None) };
}

/// Running input log. Dropping it removes the hooks and closes the file.
pub struct InputHook {
    hook_thread_id: u32,
    hook_thread: Option<JoinHandle<()>>,
    writer_thread: Option<JoinHandle<()>>,
}

impl Drop for InputHook {
    fn drop(&mut self) {
        // SAFETY: posting a message to a thread id is always safe; a dead thread just fails.
        unsafe {
            let _ = PostThreadMessageW(self.hook_thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        // The hook thread drops the sender when it ends, which ends the writer thread.
        for thread in [self.hook_thread.take(), self.writer_thread.take()].into_iter().flatten() {
            let _ = thread.join();
        }
    }
}

/// `ignored` lists key combinations that must not be logged (the app's own hotkeys). While
/// `skip_clicks` is set (the user is drawing on screen) mouse buttons are not logged, so the
/// drawing does not turn into click effects.
pub fn start_input_hook(
    log_path: &Path,
    header: Header,
    timeline: Arc<Mutex<Timeline>>,
    ignored: Vec<String>,
    skip_clicks: Arc<AtomicBool>,
) -> Result<InputHook, String> {
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Không tạo được thư mục nhật ký: {e}"))?;
    }
    let file = std::fs::File::create(log_path).map_err(|e| format!("Không tạo được file nhật ký thao tác: {e}"))?;
    let (tx, rx) = mpsc::channel::<Raw>();
    let origin = header.region;

    let writer_thread = std::thread::Builder::new()
        .name("hp-input-log".into())
        .spawn(move || write_log(BufWriter::new(file), header, origin, rx, timeline, ignored, skip_clicks))
        .map_err(|e| e.to_string())?;

    let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();
    let hook_thread = std::thread::Builder::new()
        .name("hp-input-hook".into())
        .spawn(move || run_hooks(tx, ready_tx))
        .map_err(|e| e.to_string())?;
    let hook_thread_id = ready_rx
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| "Không cài được hook chuột/phím".to_string())??;

    Ok(InputHook { hook_thread_id, hook_thread: Some(hook_thread), writer_thread: Some(writer_thread) })
}

fn run_hooks(tx: Sender<Raw>, ready: Sender<Result<u32, String>>) {
    SENDER.with(|s| *s.borrow_mut() = Some(tx));
    // SAFETY: the callbacks are valid `extern "system"` functions for the whole thread lifetime;
    // hooks are removed before the thread ends.
    unsafe {
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0);
        let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0);
        match (&mouse, &keyboard) {
            (Ok(_), Ok(_)) => {
                let _ = ready.send(Ok(GetCurrentThreadId()));
                let mut msg = MSG::default();
                // Low-level hooks need a message loop on this thread; WM_QUIT ends it.
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
            }
            _ => {
                let _ = ready.send(Err("Windows từ chối cài hook chuột/phím".to_string()));
            }
        }
        for hook in [mouse, keyboard].into_iter().flatten() {
            let _ = UnhookWindowsHookEx(hook);
        }
    }
    SENDER.with(|s| s.borrow_mut().take());
}

fn send(raw: Raw) {
    SENDER.with(|s| {
        if let Some(tx) = s.borrow().as_ref() {
            let _ = tx.send(raw);
        }
    });
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        // SAFETY: for WH_MOUSE_LL with code >= 0, lparam points to a valid MSLLHOOKSTRUCT.
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        send(Raw::Mouse {
            time: clock::now_100ns(),
            message: wparam.0 as u32,
            x: info.pt.x,
            y: info.pt.y,
            wheel: (info.mouseData >> 16) as i16 as i32,
        });
    }
    // SAFETY: always pass the event on.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        // SAFETY: for WH_KEYBOARD_LL with code >= 0, lparam points to a valid KBDLLHOOKSTRUCT.
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let down = |vk: u16| unsafe { GetAsyncKeyState(vk as i32) } < 0;
        match wparam.0 as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => send(Raw::KeyDown {
                time: clock::now_100ns(),
                key: KeyStroke {
                    ctrl: down(VK_CONTROL.0),
                    alt: down(VK_MENU.0),
                    shift: down(VK_SHIFT.0),
                    win: down(VK_LWIN.0) || down(VK_RWIN.0),
                    vk: info.vkCode,
                },
            }),
            WM_KEYUP | WM_SYSKEYUP => send(Raw::KeyUp { vk: info.vkCode }),
            _ => {}
        }
    }
    // SAFETY: always pass the event on.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn write_log(
    mut out: BufWriter<std::fs::File>,
    header: Header,
    origin: Rect,
    rx: Receiver<Raw>,
    timeline: Arc<Mutex<Timeline>>,
    ignored: Vec<String>,
    skip_clicks: Arc<AtomicBool>,
) {
    let _ = writeln!(out, "{}", to_line(&header));
    let mut sampler = MoveSampler::new();
    let mut held_key: Option<u32> = None; // suppresses auto-repeat while a key stays down
    let mut last_flush = Instant::now();
    let video_ms = |time: i64| timeline.lock().unwrap_or_else(|e| e.into_inner()).video_ms(time);

    loop {
        let raw = match rx.recv_timeout(FLUSH_INTERVAL) {
            Ok(raw) => Some(raw),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        let event = match raw {
            Some(Raw::Mouse { time, message, x, y, wheel }) => video_ms(time).and_then(|t| {
                let (x, y) = (x - origin.x, y - origin.y);
                match message {
                    WM_MOUSEMOVE => sampler.accept(t).then_some(Event::Move { t, x, y }),
                    WM_LBUTTONDOWN => Some(Event::Down { t, b: Button::Left, x, y }),
                    WM_LBUTTONUP => Some(Event::Up { t, b: Button::Left, x, y }),
                    WM_RBUTTONDOWN => Some(Event::Down { t, b: Button::Right, x, y }),
                    WM_RBUTTONUP => Some(Event::Up { t, b: Button::Right, x, y }),
                    WM_MBUTTONDOWN => Some(Event::Down { t, b: Button::Middle, x, y }),
                    WM_MBUTTONUP => Some(Event::Up { t, b: Button::Middle, x, y }),
                    WM_MOUSEWHEEL => Some(Event::Wheel { t, dx: 0, dy: wheel, x, y }),
                    WM_MOUSEHWHEEL => Some(Event::Wheel { t, dx: wheel, dy: 0, x, y }),
                    _ => None,
                }
            }),
            Some(Raw::KeyDown { time, key }) => {
                let repeat = held_key == Some(key.vk);
                held_key = Some(key.vk);
                let combo = if repeat { None } else { loggable_combo(&key).filter(|k| !ignored.contains(k)) };
                video_ms(time).zip(combo).map(|(t, k)| Event::Key { t, k })
            }
            Some(Raw::KeyUp { vk }) => {
                if held_key == Some(vk) {
                    held_key = None;
                }
                None
            }
            None => None,
        };
        if let Some(event) = event.filter(|e| keep_event(e, skip_clicks.load(Ordering::Relaxed))) {
            let _ = writeln!(out, "{}", to_line(&event));
        }
        if last_flush.elapsed() >= FLUSH_INTERVAL {
            let _ = out.flush();
            last_flush = Instant::now();
        }
    }
    let _ = out.flush();
}

/// Whether an event goes into the log: button presses are left out while the user draws.
fn keep_event(e: &Event, drawing: bool) -> bool {
    !(drawing && matches!(e, Event::Down { .. } | Event::Up { .. }))
}

#[cfg(test)]
mod filter_tests {
    use super::*;

    #[test]
    fn clicks_are_dropped_while_drawing() {
        let down = Event::Down { t: 1, b: Button::Left, x: 0, y: 0 };
        let up = Event::Up { t: 2, b: Button::Right, x: 0, y: 0 };
        let mv = Event::Move { t: 3, x: 0, y: 0 };
        let wheel = Event::Wheel { t: 4, dx: 0, dy: 120, x: 0, y: 0 };
        let key = Event::Key { t: 5, k: "Ctrl+S".into() };
        assert!(!keep_event(&down, true) && !keep_event(&up, true));
        assert!(keep_event(&mv, true) && keep_event(&wheel, true) && keep_event(&key, true));
        assert!(keep_event(&down, false) && keep_event(&up, false));
    }
}

#[cfg(test)]
mod desktop_tests {
    //! Need a real desktop session: `cargo test -p hp-screen --lib -- --ignored --nocapture`
    use super::*;
    use hp_core::input_log::LOG_VERSION;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_MOVE, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput,
    };

    fn mouse(flags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS, dx: i32, dy: i32, data: i32) {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 { mi: MOUSEINPUT { dx, dy, mouseData: data as u32, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
        };
        // SAFETY: one valid INPUT structure of the declared size.
        unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
    }

    #[test]
    #[ignore = "needs a desktop session; moves the mouse by a few pixels"]
    fn synthetic_mouse_input_is_logged_with_header_first() {
        let path = std::env::temp_dir().join("hp-screen-tests").join("input-test.input.jsonl");
        let header = Header {
            v: LOG_VERSION,
            mode: "monitor".into(),
            region: Rect { x: 0, y: 0, w: 1920, h: 1080 },
            width: 1920,
            height: 1080,
            fps: 30,
        };
        let timeline = Arc::new(Mutex::new(Timeline::new(clock::now_100ns(), clock::TICKS_PER_SEC)));
        let hook = start_input_hook(&path, header, timeline.clone(), Vec::new(), Arc::new(AtomicBool::new(false))).expect("hooks install");
        std::thread::sleep(Duration::from_millis(200));
        mouse(MOUSEEVENTF_MOVE, 3, 0, 0);
        std::thread::sleep(Duration::from_millis(50));
        mouse(MOUSEEVENTF_WHEEL, 0, 0, -120);
        std::thread::sleep(Duration::from_millis(50));
        // Events while paused must be left out.
        timeline.lock().unwrap().pause(clock::now_100ns());
        mouse(MOUSEEVENTF_WHEEL, 0, 0, 120);
        std::thread::sleep(Duration::from_millis(50));
        timeline.lock().unwrap().resume(clock::now_100ns());
        mouse(MOUSEEVENTF_MOVE, -3, 0, 0);
        std::thread::sleep(Duration::from_millis(200));
        drop(hook);

        let text = std::fs::read_to_string(&path).unwrap();
        println!("{text}");
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with(r#"{"v":1,"mode":"monitor""#));
        assert!(lines.iter().any(|l| l.contains(r#""e":"move""#)));
        assert_eq!(lines.iter().filter(|l| l.contains(r#""e":"wheel""#)).count(), 1, "the wheel event during pause must be skipped");
        assert!(lines.iter().any(|l| l.contains(r#""dy":-120"#)));
    }
}
