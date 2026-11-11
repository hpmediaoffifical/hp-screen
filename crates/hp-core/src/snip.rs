//! Pure logic for "capture and annotate": snapping the selection to windows, choosing the OCR
//! language and picture size, and stitching a scrolling capture into one long picture.
use crate::geometry::Rect;

/// Windows smaller than this (physical pixels, either side) are not offered for snapping.
pub const MIN_SNAP: u32 = 24;

/// Window rectangles in virtual-screen pixels, top-most first, turned into rectangles relative
/// to `monitor` and clipped to it. Each result keeps the index of the window it came from;
/// windows with no visible part on the monitor, or too small to aim at, are left out.
pub fn snap_targets(windows: &[Rect], monitor: Rect) -> Vec<(usize, Rect)> {
    let (m_left, m_top) = (monitor.x as i64, monitor.y as i64);
    let (m_right, m_bottom) = (m_left + monitor.w as i64, m_top + monitor.h as i64);
    windows
        .iter()
        .enumerate()
        .filter_map(|(i, w)| {
            let left = (w.x as i64).max(m_left);
            let top = (w.y as i64).max(m_top);
            let right = (w.x as i64 + w.w as i64).min(m_right);
            let bottom = (w.y as i64 + w.h as i64).min(m_bottom);
            if right - left < MIN_SNAP as i64 || bottom - top < MIN_SNAP as i64 {
                return None;
            }
            let rect = Rect { x: (left - m_left) as i32, y: (top - m_top) as i32, w: (right - left) as u32, h: (bottom - top) as u32 };
            Some((i, rect))
        })
        .collect()
}

/// The language the OCR engine should read, from the ones installed (`available`, BCP-47 tags
/// such as "en-US"): Vietnamese first, then English, then the user's own languages in order,
/// then whatever is installed. `None` when no OCR language is installed at all.
pub fn pick_ocr_language(available: &[String], user_languages: &[String]) -> Option<usize> {
    let primary = |tag: &str| tag.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    let find = |wanted: &str| available.iter().position(|a| primary(a) == primary(wanted));
    find("vi")
        .or_else(|| find("en"))
        .or_else(|| user_languages.iter().find_map(|u| find(u)))
        .or(if available.is_empty() { None } else { Some(0) })
}

/// Largest picture the OCR step works on, in pixels; bigger ones only make it slower.
pub const OCR_MAX_PIXELS: f64 = 16_000_000.0;

/// How much to scale a `width` × `height` picture before OCR. Screen text is small, and the
/// engine reads it better when enlarged, so pictures are doubled when there is room; pictures
/// longer than the engine's `max_dimension` are shrunk to fit.
pub fn ocr_scale(width: u32, height: u32, max_dimension: u32) -> f64 {
    let longest = width.max(height).max(1) as f64;
    let area = (width.max(1) as f64) * (height.max(1) as f64);
    let enlarge = (OCR_MAX_PIXELS / area).sqrt().clamp(1.0, 2.0);
    enlarge.min(max_dimension as f64 / longest)
}

/// What happened to one frame given to a [`Stitcher`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StitchStep {
    /// The first frame: the picture starts with it.
    Started,
    /// Same content as the last frame (nothing scrolled).
    Unchanged,
    /// The page scrolled down by this many pixels; that many new rows were added.
    Added(usize),
    /// The frame does not continue the last one (scrolled up, or too far at once). Ignored.
    NoMatch,
    /// The picture reached its height limit; later frames are ignored.
    Full,
}

/// Joins frames of a region captured while the user scrolls down into one tall picture.
///
/// Each frame is compared with the last accepted one to find how far the page moved, using
/// the rows they share; only the rows that came into view are added. Rows that stay put at
/// the top and bottom (a sticky header or footer) are kept once: the header from the first
/// frame, the footer from the last. Rows are compared only on the columns that scroll: a fixed
/// side menu, and the scrollbar whose handle moves, are left out of the comparison.
pub struct Stitcher {
    width: usize,
    height: usize,
    max_height: usize,
    /// Accepted rows so far, RGBA, without the footer.
    out: Vec<u8>,
    last: Vec<u8>,
    last_rows: Vec<RowSig>,
    /// Fixed rows at the top and bottom, known after the first scroll.
    bands: Option<(usize, usize)>,
    /// Columns compared (start..end), known after the first scroll with the bands.
    cols: (usize, usize),
    full: bool,
}

/// Signature of one pixel row: its hash, and whether the row has any detail at all (a row of
/// a single colour matches every other plain row, so it says nothing about the scroll).
#[derive(Clone, Copy, PartialEq, Eq)]
struct RowSig {
    hash: u64,
    detailed: bool,
}

const BYTES_PER_PIXEL: usize = 4;
/// Share of the compared rows that must be equal for a scroll offset to count.
const MATCH_RATIO: f64 = 0.9;
/// Detailed rows that must match before an offset is believed.
const MIN_MATCHES: usize = 3;
/// Columns left out at the right edge of the scrolling part, where a scrollbar usually is
/// (its handle moves with the page, so those columns never line up).
const SCROLLBAR_COLS: usize = 24;

impl Stitcher {
    /// `width` × `height` is the size of every frame; the result is never taller than `max_height`.
    pub fn new(width: usize, height: usize, max_height: usize) -> Self {
        Self {
            width,
            height,
            max_height: max_height.max(height),
            out: Vec::new(),
            last: Vec::new(),
            last_rows: Vec::new(),
            bands: None,
            cols: (0, width),
            full: false,
        }
    }

    fn row_bytes(&self) -> usize {
        self.width * BYTES_PER_PIXEL
    }

    fn signatures(&self, rgba: &[u8], (start, end): (usize, usize)) -> Vec<RowSig> {
        rgba.chunks(self.row_bytes())
            .take(self.height)
            .map(|row| {
                let row = &row[start * BYTES_PER_PIXEL..end * BYTES_PER_PIXEL];
                // FNV-1a.
                let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
                for &b in row {
                    hash = (hash ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
                }
                let first = &row[..BYTES_PER_PIXEL.min(row.len())];
                let detailed = row.chunks(BYTES_PER_PIXEL).any(|px| px != first);
                RowSig { hash, detailed }
            })
            .collect()
    }

    /// Adds one frame (`width * height * 4` RGBA bytes).
    pub fn push(&mut self, rgba: &[u8]) -> StitchStep {
        if rgba.len() < self.row_bytes() * self.height {
            return StitchStep::NoMatch;
        }
        let rgba = &rgba[..self.row_bytes() * self.height];
        if self.last.is_empty() {
            self.out = rgba.to_vec();
            self.last = rgba.to_vec();
            self.last_rows = self.signatures(rgba, self.cols);
            return StitchStep::Started;
        }
        if self.full {
            return StitchStep::Full;
        }
        if rgba == self.last.as_slice() {
            return StitchStep::Unchanged;
        }
        // Until the first scroll is found, the fixed parts are guessed again from each frame.
        let (cols, last_rows) = match self.bands {
            Some(_) => (self.cols, std::mem::take(&mut self.last_rows)),
            None => {
                let cols = self.moving_columns(rgba);
                (cols, self.signatures(&self.last, cols))
            }
        };
        let rows = self.signatures(rgba, cols);
        let (top, bottom) = match self.bands {
            Some(bands) => bands,
            None => {
                let top = rows.iter().zip(&last_rows).take_while(|(a, b)| a == b).count();
                let bottom = rows.iter().rev().zip(last_rows.iter().rev()).take_while(|(a, b)| a == b).count();
                // Keep a band in the middle tall enough to compare: never treat most of the frame as fixed.
                let limit = self.height / 3;
                (top.min(limit), bottom.min(limit))
            }
        };
        let zone = top..self.height - bottom;
        let found = find_offset(&last_rows[zone.clone()], &rows[zone.clone()]);
        if self.bands.is_some() {
            self.last_rows = last_rows;
        }
        let Some(offset) = found else {
            return StitchStep::NoMatch;
        };
        if offset == 0 {
            return StitchStep::Unchanged;
        }
        if self.bands.is_none() {
            // The first frame still holds its footer; it goes back on at the end, from the last frame.
            self.bands = Some((top, bottom));
            self.cols = cols;
            self.out.truncate((self.height - bottom) * self.row_bytes());
        }
        let current = self.out.len() / self.row_bytes() + bottom;
        let room = self.max_height.saturating_sub(current);
        let added = offset.min(room);
        let new_rows = (zone.end - offset)..(zone.end - offset + added);
        self.out.extend_from_slice(&rgba[new_rows.start * self.row_bytes()..new_rows.end * self.row_bytes()]);
        self.last = rgba.to_vec();
        self.last_rows = rows;
        if added < offset {
            self.full = true;
            return if added == 0 { StitchStep::Full } else { StitchStep::Added(added) };
        }
        StitchStep::Added(added)
    }

    /// Columns worth comparing between the last frame and `next`: without the columns that are
    /// the same in both (a fixed side menu, or empty margins) and without the scrollbar strip
    /// at the right of what is left.
    fn moving_columns(&self, next: &[u8]) -> (usize, usize) {
        let (w, h) = (self.width, self.height);
        let same = |x: usize| {
            (0..h).all(|y| {
                let i = (y * w + x) * BYTES_PER_PIXEL;
                self.last[i..i + BYTES_PER_PIXEL] == next[i..i + BYTES_PER_PIXEL]
            })
        };
        let mut start = 0;
        while start < w / 2 && same(start) {
            start += 1;
        }
        let mut end = w;
        while end > w / 2 && end - start > w / 4 && same(end - 1) {
            end -= 1;
        }
        end -= SCROLLBAR_COLS.min((end - start) / 8);
        (start, end.max(start + 1))
    }

    /// Height of the picture so far, in rows.
    pub fn height(&self) -> usize {
        let footer = self.bands.map_or(0, |(_, bottom)| bottom);
        self.out.len() / self.row_bytes().max(1) + footer
    }

    /// The finished picture: `(width, height, rgba)`. Empty when no frame was given.
    pub fn finish(mut self) -> (usize, usize, Vec<u8>) {
        if let Some((_, bottom)) = self.bands {
            let start = (self.height - bottom) * self.row_bytes();
            let footer = self.last[start..].to_vec();
            self.out.extend_from_slice(&footer);
        }
        let height = self.out.len() / self.row_bytes().max(1);
        (self.width, height, self.out)
    }
}

/// How far `next` is scrolled down from `last` (rows of the same band), if they overlap.
fn find_offset(last: &[RowSig], next: &[RowSig]) -> Option<usize> {
    let n = last.len().min(next.len());
    if n == 0 {
        return None;
    }
    // Both frames must share a fair part of the band to be trusted.
    let min_overlap = (n / 8).max(MIN_MATCHES * 2).min(n);
    let mut best: Option<(usize, usize)> = None; // (matches, offset)
    for offset in 0..=(n - min_overlap) {
        let overlap = n - offset;
        let (mut matches, mut compared) = (0, 0);
        for i in 0..overlap {
            let (a, b) = (last[i + offset], next[i]);
            if !a.detailed && !b.detailed {
                continue;
            }
            compared += 1;
            if a == b {
                matches += 1;
            }
        }
        if matches < MIN_MATCHES || (matches as f64) < compared as f64 * MATCH_RATIO {
            continue;
        }
        if best.is_none_or(|(m, _)| matches > m) {
            best = Some((matches, offset));
        }
    }
    best.map(|(_, offset)| offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITOR: Rect = Rect { x: 0, y: 0, w: 1920, h: 1080 };
    const LEFT: Rect = Rect { x: -1920, y: 0, w: 1920, h: 1080 };

    #[test]
    fn snap_targets_are_relative_to_the_monitor_and_clipped_to_it() {
        let windows = [
            Rect { x: 100, y: 50, w: 800, h: 600 },
            Rect { x: -1820, y: 10, w: 400, h: 300 },
            Rect { x: 1700, y: 900, w: 500, h: 400 },
        ];
        assert_eq!(
            snap_targets(&windows, MONITOR),
            vec![(0, Rect { x: 100, y: 50, w: 800, h: 600 }), (2, Rect { x: 1700, y: 900, w: 220, h: 180 })]
        );
        assert_eq!(snap_targets(&windows, LEFT), vec![(1, Rect { x: 100, y: 10, w: 400, h: 300 })]);
    }

    #[test]
    fn snap_targets_skip_windows_too_small_or_barely_on_screen() {
        let windows = [
            Rect { x: 10, y: 10, w: 10, h: 300 },
            Rect { x: 1910, y: 10, w: 400, h: 300 }, // only 10 px on this monitor
            Rect { x: -8, y: -8, w: 1936, h: 1096 }, // maximized: frame hangs over the edges
        ];
        assert_eq!(snap_targets(&windows, MONITOR), vec![(2, MONITOR)]);
    }

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn ocr_prefers_vietnamese_then_english() {
        assert_eq!(pick_ocr_language(&tags(&["fr-FR", "en-US", "vi-VN"]), &tags(&["fr-FR"])), Some(2));
        assert_eq!(pick_ocr_language(&tags(&["fr-FR", "en-GB"]), &tags(&["fr-FR"])), Some(1));
        assert_eq!(pick_ocr_language(&tags(&["vi"]), &[]), Some(0));
    }

    #[test]
    fn ocr_falls_back_to_the_users_languages_then_anything_installed() {
        assert_eq!(pick_ocr_language(&tags(&["de-DE", "ja-JP"]), &tags(&["ko-KR", "ja-JP"])), Some(1));
        assert_eq!(pick_ocr_language(&tags(&["de-DE", "ja-JP"]), &tags(&["ko-KR"])), Some(0));
        assert_eq!(pick_ocr_language(&[], &tags(&["vi-VN"])), None);
    }

    #[test]
    fn ocr_enlarges_small_pictures_and_shrinks_huge_ones() {
        assert_eq!(ocr_scale(400, 100, 10_000), 2.0);
        // 4K screen: 33 MP when doubled, so enlarged only up to the 16 MP budget.
        let s = ocr_scale(3840, 2160, 10_000);
        assert!((3840.0 * s * 2160.0 * s - OCR_MAX_PIXELS).abs() < 1.0, "{s}");
        // Already over the budget: kept as it is, never shrunk for the budget alone.
        assert_eq!(ocr_scale(7680, 4320, 10_000), 1.0);
        // Older engines read at most 2600 px on the long side.
        assert!((ocr_scale(5200, 300, 2600) - 0.5).abs() < 1e-9);
        assert_eq!(ocr_scale(0, 0, 2600), 2.0);
    }

    // ---- scrolling capture ----

    const W: usize = 6;

    /// A tall "page": every row distinct, with some plain (blank) stretches like real pages.
    fn page(rows: usize) -> Vec<[u8; 4]> {
        (0..rows)
            .flat_map(|y| {
                (0..W).map(move |x| {
                    if (y / 20) % 4 == 3 {
                        [255, 255, 255, 255] // blank stretch
                    } else {
                        [(y * 7 + x) as u8, (y / 3) as u8, (y * 13 % 251) as u8, 255]
                    }
                })
            })
            .collect()
    }

    fn rows_of(page: &[[u8; 4]], from: usize, count: usize) -> Vec<u8> {
        page[from * W..(from + count) * W].iter().flatten().copied().collect()
    }

    /// A frame of `h` rows of `page` starting at `scroll`, with fixed `header` and `footer` rows on top.
    fn frame(page: &[[u8; 4]], scroll: usize, h: usize, header: usize, footer: usize) -> Vec<u8> {
        let mut out = vec![9u8; header * W * 4];
        out.extend(rows_of(page, scroll, h - header - footer));
        out.extend(vec![200u8; footer * W * 4]);
        out
    }

    #[test]
    fn stitches_frames_scrolled_by_different_amounts() {
        let p = page(600);
        let h = 120;
        let mut s = Stitcher::new(W, h, 10_000);
        assert_eq!(s.push(&frame(&p, 0, h, 0, 0)), StitchStep::Started);
        assert_eq!(s.push(&frame(&p, 37, h, 0, 0)), StitchStep::Added(37));
        assert_eq!(s.push(&frame(&p, 37, h, 0, 0)), StitchStep::Unchanged);
        assert_eq!(s.push(&frame(&p, 80, h, 0, 0)), StitchStep::Added(43));
        assert_eq!(s.push(&frame(&p, 150, h, 0, 0)), StitchStep::Added(70));
        assert_eq!(s.height(), 150 + h);
        let (w, height, rgba) = s.finish();
        assert_eq!((w, height), (W, 150 + h));
        assert_eq!(rgba, rows_of(&p, 0, 150 + h));
    }

    #[test]
    fn sticky_header_and_footer_appear_once() {
        let p = page(600);
        let (h, header, footer) = (120, 10, 8);
        let mut s = Stitcher::new(W, h, 10_000);
        s.push(&frame(&p, 0, h, header, footer));
        assert_eq!(s.push(&frame(&p, 25, h, header, footer)), StitchStep::Added(25));
        assert_eq!(s.push(&frame(&p, 60, h, header, footer)), StitchStep::Added(35));
        let (_, height, rgba) = s.finish();
        let body = h - header - footer;
        assert_eq!(height, header + 60 + body + footer);
        let mut expected = vec![9u8; header * W * 4];
        expected.extend(rows_of(&p, 0, 60 + body));
        expected.extend(vec![200u8; footer * W * 4]);
        assert_eq!(rgba, expected);
    }

    #[test]
    fn frames_that_do_not_continue_the_page_are_ignored() {
        let p = page(900);
        let h = 120;
        let mut s = Stitcher::new(W, h, 10_000);
        s.push(&frame(&p, 100, h, 0, 0));
        assert_eq!(s.push(&frame(&p, 60, h, 0, 0)), StitchStep::NoMatch, "scrolled up");
        assert_eq!(s.push(&frame(&p, 400, h, 0, 0)), StitchStep::NoMatch, "jumped past the frame");
        assert_eq!(s.push(&frame(&p, 150, h, 0, 0)), StitchStep::Added(50));
        let (_, height, rgba) = s.finish();
        assert_eq!(rgba, rows_of(&p, 100, 50 + h));
        assert_eq!(height, 50 + h);
    }

    #[test]
    fn a_blank_page_never_seems_to_scroll() {
        let blank = vec![255u8; W * 4 * 100];
        let mut s = Stitcher::new(W, 100, 10_000);
        s.push(&blank);
        assert_eq!(s.push(&blank), StitchStep::Unchanged);
        let mut almost = blank.clone();
        almost[0] = 0; // one detailed row, nothing to line it up with
        assert_eq!(s.push(&almost), StitchStep::NoMatch);
    }

    #[test]
    fn the_picture_stops_at_its_height_limit() {
        let p = page(900);
        let h = 100;
        let mut s = Stitcher::new(W, h, 160);
        s.push(&frame(&p, 0, h, 0, 0));
        assert_eq!(s.push(&frame(&p, 40, h, 0, 0)), StitchStep::Added(40));
        assert_eq!(s.push(&frame(&p, 80, h, 0, 0)), StitchStep::Added(20));
        assert_eq!(s.push(&frame(&p, 120, h, 0, 0)), StitchStep::Full);
        let (_, height, rgba) = s.finish();
        assert_eq!(height, 160);
        assert_eq!(rgba, rows_of(&p, 0, 160));
    }

    /// A window with a fixed side menu on the left and a scrollbar whose handle moves on the right.
    fn window_frame(page: &[[u8; 4]], scroll: usize, h: usize) -> Vec<u8> {
        const WIDE: usize = 20;
        const MENU: usize = 4;
        let mut out = Vec::new();
        for y in 0..h {
            for x in 0..WIDE {
                let px = if x < MENU {
                    [(y * 5 + x * 31) as u8, 40, (y % 7 * 30) as u8, 255] // same at every scroll
                } else if x == WIDE - 1 {
                    let handle = scroll / 4..scroll / 4 + 25;
                    if handle.contains(&y) { [90, 90, 90, 255] } else { [230, 230, 230, 255] }
                } else {
                    page[(scroll + y) * W + (x - MENU) % W]
                };
                out.extend_from_slice(&px);
            }
        }
        out
    }

    #[test]
    fn a_fixed_side_menu_and_a_moving_scrollbar_do_not_stop_the_stitching() {
        let p = page(700);
        let h = 120;
        let mut s = Stitcher::new(20, h, 10_000);
        s.push(&window_frame(&p, 0, h));
        assert_eq!(s.push(&window_frame(&p, 30, h)), StitchStep::Added(30));
        assert_eq!(s.push(&window_frame(&p, 75, h)), StitchStep::Added(45));
        let (_, height, rgba) = s.finish();
        assert_eq!(height, 75 + h);
        // The scrolling part of every row is the page, in order.
        for y in 0..height {
            let px = &rgba[(y * 20 + 4) * 4..(y * 20 + 5) * 4];
            assert_eq!(px, &p[y * W], "row {y}");
        }
    }

    #[test]
    fn short_or_wrong_sized_frames_are_ignored() {
        let mut s = Stitcher::new(W, 10, 100);
        assert_eq!(s.push(&[0u8; 8]), StitchStep::NoMatch);
        let (_, height, rgba) = s.finish();
        assert_eq!((height, rgba.len()), (0, 0));
    }
}
