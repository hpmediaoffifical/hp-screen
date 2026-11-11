//! The one clock video, audio and the input log share: QueryPerformanceCounter expressed in
//! 100 ns units, which is also what Windows Graphics Capture stamps frames with.
use std::sync::OnceLock;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

pub const TICKS_PER_SEC: i64 = 10_000_000;

pub fn now_100ns() -> i64 {
    static FREQUENCY: OnceLock<i64> = OnceLock::new();
    let frequency = *FREQUENCY.get_or_init(|| {
        let mut f = 0i64;
        // SAFETY: the out pointer is valid; the call cannot fail on Windows XP and later.
        let _ = unsafe { QueryPerformanceFrequency(&mut f) };
        f.max(1)
    });
    let mut counter = 0i64;
    // SAFETY: the out pointer is valid.
    let _ = unsafe { QueryPerformanceCounter(&mut counter) };
    (counter as i128 * TICKS_PER_SEC as i128 / frequency as i128) as i64
}
