//! Rectangles in virtual-screen pixel coordinates.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// Smallest width or height a capture region may have.
pub const MIN_SIZE: u32 = 32;

/// H.264 needs even dimensions: odd values lose one pixel.
pub fn make_even(w: u32, h: u32) -> (u32, u32) {
    (w & !1, h & !1)
}

/// Clamps `region` to `monitor` and returns it relative to the monitor's origin with even
/// dimensions. `None` when the visible part is smaller than `MIN_SIZE` on either side.
pub fn crop_in_monitor(region: Rect, monitor: Rect) -> Option<Rect> {
    let clamped = clamp_in_monitor(region, monitor)?;
    let (w, h) = make_even(clamped.w, clamped.h);
    if w < MIN_SIZE || h < MIN_SIZE {
        return None;
    }
    Some(Rect { w, h, ..clamped })
}

/// Like `crop_in_monitor` but keeps odd sizes: screenshots need every physical pixel.
pub fn clamp_in_monitor(region: Rect, monitor: Rect) -> Option<Rect> {
    let (left, top, right, bottom) = edges(region);
    let (m_left, m_top, m_right, m_bottom) = edges(monitor);
    let left = left.max(m_left);
    let top = top.max(m_top);
    let right = right.min(m_right);
    let bottom = bottom.min(m_bottom);
    if right - left < MIN_SIZE as i64 || bottom - top < MIN_SIZE as i64 {
        return None;
    }
    Some(Rect {
        x: (left - m_left) as i32,
        y: (top - m_top) as i32,
        w: (right - left) as u32,
        h: (bottom - top) as u32,
    })
}

/// Index of the monitor containing `point`.
pub fn monitor_containing(point: (i32, i32), monitors: &[Rect]) -> Option<usize> {
    let (px, py) = (point.0 as i64, point.1 as i64);
    monitors.iter().position(|m| {
        let (left, top, right, bottom) = edges(*m);
        px >= left && px < right && py >= top && py < bottom
    })
}

/// (left, top, right, bottom) in i64 so large coordinates cannot overflow.
fn edges(r: Rect) -> (i64, i64, i64, i64) {
    let (x, y) = (r.x as i64, r.y as i64);
    (x, y, x + r.w as i64, y + r.h as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN: Rect = Rect { x: 0, y: 0, w: 1920, h: 1080 };
    const LEFT: Rect = Rect { x: -1920, y: 0, w: 1920, h: 1080 };

    #[test]
    fn make_even_drops_one_pixel_from_odd_sizes() {
        assert_eq!(make_even(1921, 1080), (1920, 1080));
        assert_eq!(make_even(800, 601), (800, 600));
    }

    #[test]
    fn crop_makes_odd_region_even() {
        assert_eq!(
            crop_in_monitor(Rect { x: 100, y: 50, w: 801, h: 601 }, MAIN),
            Some(Rect { x: 100, y: 50, w: 800, h: 600 })
        );
    }

    #[test]
    fn crop_is_relative_to_monitor_with_negative_origin() {
        assert_eq!(
            crop_in_monitor(Rect { x: -1820, y: 10, w: 400, h: 300 }, LEFT),
            Some(Rect { x: 100, y: 10, w: 400, h: 300 })
        );
    }

    #[test]
    fn crop_clamps_region_overflowing_the_monitor_edge() {
        assert_eq!(
            crop_in_monitor(Rect { x: 1800, y: 1000, w: 400, h: 300 }, MAIN),
            Some(Rect { x: 1800, y: 1000, w: 120, h: 80 })
        );
        assert_eq!(
            crop_in_monitor(Rect { x: -50, y: -20, w: 250, h: 220 }, MAIN),
            Some(Rect { x: 0, y: 0, w: 200, h: 200 })
        );
    }

    #[test]
    fn clamp_keeps_odd_sizes_for_screenshots() {
        assert_eq!(
            clamp_in_monitor(Rect { x: 100, y: 50, w: 801, h: 601 }, MAIN),
            Some(Rect { x: 100, y: 50, w: 801, h: 601 })
        );
        assert_eq!(
            clamp_in_monitor(Rect { x: -1820, y: 1000, w: 401, h: 300 }, LEFT),
            Some(Rect { x: 100, y: 1000, w: 401, h: 80 })
        );
        assert_eq!(clamp_in_monitor(Rect { x: 10, y: 10, w: 20, h: 500 }, MAIN), None);
        assert_eq!(clamp_in_monitor(Rect { x: 5000, y: 0, w: 400, h: 300 }, MAIN), None);
    }

    #[test]
    fn crop_rejects_region_smaller_than_minimum() {
        assert_eq!(crop_in_monitor(Rect { x: 10, y: 10, w: 20, h: 500 }, MAIN), None);
    }

    #[test]
    fn crop_rejects_region_outside_monitor() {
        assert_eq!(crop_in_monitor(Rect { x: 5000, y: 0, w: 400, h: 300 }, MAIN), None);
    }

    #[test]
    fn monitor_containing_finds_monitor_left_of_primary() {
        assert_eq!(monitor_containing((-10, 5), &[MAIN, LEFT]), Some(1));
        assert_eq!(monitor_containing((0, 0), &[MAIN, LEFT]), Some(0));
    }

    #[test]
    fn monitor_containing_is_none_outside_all_monitors() {
        assert_eq!(monitor_containing((9999, 5), &[MAIN, LEFT]), None);
        assert_eq!(monitor_containing((1920, 5), &[MAIN, LEFT]), None);
    }
}
