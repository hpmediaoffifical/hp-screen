/*! © 2026 HP Media (hpvn.media) — HP Screen. Bảo lưu mọi quyền. HP không có đại lý khác, không hợp tác với đơn vị nào, mọi sao chép đều là vi phạm. */
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

/// One word the OCR engine read, and where it sits in the picture (pixels).
#[derive(Clone, Copy, Debug)]
pub struct OcrBox<'a> {
    pub text: &'a str,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Text that stood in columns, as a spreadsheet takes it from the clipboard: the cells of a
/// row separated by tabs, the rows by line breaks.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct TableText {
    pub text: String,
    pub rows: usize,
    pub cols: usize,
}

/// Words closer than this (in text heights) belong to one cell; a wider space starts another.
const CELL_GAP: f64 = 1.2;
/// A column boundary is an empty strip at least this wide (in text heights) across every row
/// that has several cells.
const COLUMN_STRIP: f64 = 0.3;

/// Reads `words` as a table when they stand in columns, and returns `None` for ordinary text.
///
/// Words level with each other are a row. Within a row, words with a wide space between them
/// are different cells. A column boundary is a vertical strip that no cell of any several-cell
/// row crosses; rows of one cell (a title, a sentence) take no part in that, since they may run
/// across the whole table. It is a table when at least two rows have text in two columns.
/// A cell whose text wraps over several lines comes out as several rows.
pub fn table_text(words: &[OcrBox]) -> Option<TableText> {
    struct Cell {
        x0: f64,
        x1: f64,
        text: String,
    }

    let mut items: Vec<&OcrBox> = words.iter().filter(|w| !w.text.trim().is_empty() && w.w > 0.0 && w.h > 0.0).collect();
    if items.len() < 4 {
        return None;
    }
    let mut heights: Vec<f64> = items.iter().map(|w| w.h).collect();
    heights.sort_by(f64::total_cmp);
    let height = heights[heights.len() / 2];

    // Rows: words whose middles are level, top to bottom.
    let middle = |w: &OcrBox| w.y + w.h / 2.0;
    items.sort_by(|a, b| middle(a).total_cmp(&middle(b)));
    let mut lines: Vec<Vec<&OcrBox>> = Vec::new();
    let mut level = 0.0;
    for word in items {
        match lines.last_mut() {
            Some(line) if (middle(word) - level).abs() < height * 0.5 => {
                line.push(word);
                level += (middle(word) - level) / line.len() as f64;
            }
            _ => {
                level = middle(word);
                lines.push(vec![word]);
            }
        }
    }

    // Cells: runs of words with only ordinary spaces between them.
    let rows: Vec<Vec<Cell>> = lines
        .into_iter()
        .map(|mut line| {
            line.sort_by(|a, b| a.x.total_cmp(&b.x));
            let mut cells: Vec<Cell> = Vec::new();
            for word in line {
                match cells.last_mut() {
                    Some(cell) if word.x - cell.x1 < height * CELL_GAP => {
                        cell.x1 = cell.x1.max(word.x + word.w);
                        cell.text.push(' ');
                        cell.text.push_str(word.text.trim());
                    }
                    _ => cells.push(Cell { x0: word.x, x1: word.x + word.w, text: word.text.trim().to_string() }),
                }
            }
            cells
        })
        .collect();

    // Columns: what is left between the strips no cell of a several-cell row crosses.
    let mut spans: Vec<(f64, f64)> = rows.iter().filter(|r| r.len() > 1).flatten().map(|c| (c.x0, c.x1)).collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut columns: Vec<(f64, f64)> = Vec::new();
    for (x0, x1) in spans {
        match columns.last_mut() {
            Some(column) if x0 < column.1 + height * COLUMN_STRIP => column.1 = column.1.max(x1),
            _ => columns.push((x0, x1)),
        }
    }
    if columns.len() < 2 {
        return None;
    }
    // A cell belongs to the column its start is in; one that starts in a strip, to the next.
    let column_of = |cell: &Cell| columns.iter().position(|c| cell.x0 < c.1).unwrap_or(columns.len() - 1);

    let mut tabled = 0;
    let mut out: Vec<String> = Vec::with_capacity(rows.len());
    for row in &rows {
        let mut cells = vec![String::new(); columns.len()];
        for cell in row {
            let slot = &mut cells[column_of(cell)];
            if !slot.is_empty() {
                slot.push(' ');
            }
            slot.push_str(&cell.text);
        }
        if cells.iter().filter(|c| !c.is_empty()).count() >= 2 {
            tabled += 1;
        }
        out.push(cells.join("\t").trim_end_matches('\t').to_string());
    }
    if tabled < 2 {
        return None;
    }
    Some(TableText { text: out.join("\n"), rows: out.len(), cols: columns.len() })
}

#[cfg(test)]
mod table_tests {
    use super::*;

    /// Words laid out like a screen: 10 px per letter, 16 px tall, a 6 px space between words.
    fn words(rows: &[(f64, &[(f64, &'static str)])]) -> Vec<OcrBox<'static>> {
        let mut out = Vec::new();
        for (y, cells) in rows {
            for (x, text) in cells.iter() {
                let mut at = *x;
                for word in text.split(' ') {
                    let w = word.chars().count() as f64 * 10.0;
                    out.push(OcrBox { text: word, x: at, y: *y, w, h: 16.0 });
                    at += w + 6.0;
                }
            }
        }
        out
    }

    #[test]
    fn columns_become_tabs_and_rows_become_lines() {
        let w = words(&[
            (10.0, &[(10.0, "Tên hàng"), (220.0, "Số lượng"), (400.0, "Giá")]),
            (40.0, &[(10.0, "Bút bi xanh"), (220.0, "12"), (400.0, "36.000")]),
            (70.0, &[(10.0, "Vở"), (220.0, "5"), (400.0, "50.000")]),
        ]);
        let t = table_text(&w).expect("a table");
        assert_eq!((t.rows, t.cols), (3, 3));
        assert_eq!(t.text, "Tên hàng\tSố lượng\tGiá\nBút bi xanh\t12\t36.000\nVở\t5\t50.000");
    }

    #[test]
    fn an_empty_cell_keeps_its_place() {
        let w = words(&[
            (10.0, &[(10.0, "A"), (200.0, "B"), (400.0, "C")]),
            (40.0, &[(10.0, "một"), (400.0, "ba")]),
            (70.0, &[(200.0, "hai"), (400.0, "ba")]),
        ]);
        let t = table_text(&w).expect("a table");
        assert_eq!(t.text, "A\tB\tC\nmột\t\tba\n\thai\tba");
    }

    #[test]
    fn a_title_across_the_table_does_not_hide_its_columns() {
        let w = words(&[
            (10.0, &[(10.0, "Bảng giá tháng mười một năm nay cho mọi cửa hàng")]),
            (40.0, &[(10.0, "Bút"), (300.0, "3.000")]),
            (70.0, &[(10.0, "Vở"), (300.0, "10.000")]),
        ]);
        let t = table_text(&w).expect("a table");
        assert_eq!((t.rows, t.cols), (3, 2));
        assert!(t.text.starts_with("Bảng giá tháng mười một năm nay cho mọi cửa hàng\nBút\t3.000"));
    }

    #[test]
    fn ordinary_text_is_not_a_table() {
        let w = words(&[
            (10.0, &[(10.0, "HP Screen quay màn hình và chụp ảnh")]),
            (40.0, &[(10.0, "rồi cho bạn vẽ ghi chú lên ảnh")]),
            (70.0, &[(10.0, "trước khi lưu hoặc sao chép")]),
        ]);
        assert_eq!(table_text(&w), None);
        assert_eq!(table_text(&[]), None);
    }

    #[test]
    fn one_wide_space_in_one_line_is_not_a_table() {
        let w = words(&[
            (10.0, &[(10.0, "Tổng cộng"), (300.0, "86.000")]),
            (40.0, &[(10.0, "Cảm ơn quý khách đã mua hàng tại đây")]),
            (70.0, &[(10.0, "Hẹn gặp lại quý khách lần sau nhé")]),
        ]);
        assert_eq!(table_text(&w), None);
    }

    #[test]
    fn rows_that_do_not_line_up_into_columns_are_not_a_table() {
        // Every strip one row leaves empty is crossed by the other row.
        let w = words(&[
            (10.0, &[(10.0, "aaaaaaaaaa"), (200.0, "bbbbbbbbbb")]),
            (40.0, &[(100.0, "cccccccccccc"), (300.0, "dd")]),
        ]);
        assert_eq!(table_text(&w), None);
    }
}

/// One word from Tesseract's TSV output: its text, its box in the pixels of the picture that
/// was read, and which line of text it belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct TsvWord {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Block, paragraph and line numbers: words sharing all three are one line.
    pub line: (u32, u32, u32),
}

/// Words Tesseract is less sure of than this (0–100) are left out: at that level it is the
/// shapes of icons and borders read as letters, not text.
pub const TSV_MIN_CONFIDENCE: f64 = 10.0;

/// The words in Tesseract's TSV output, in reading order. Each row is
/// `level page block paragraph line word left top width height confidence text`; rows of
/// level 5 are words. Rows that do not fit that shape, empty words and words below
/// [`TSV_MIN_CONFIDENCE`] are skipped.
pub fn tsv_words(tsv: &str) -> Vec<TsvWord> {
    tsv.lines()
        .filter_map(|row| {
            let cols: Vec<&str> = row.splitn(12, '\t').collect();
            if cols.len() < 12 || cols[0] != "5" {
                return None;
            }
            let number = |i: usize| cols[i].trim().parse::<f64>().ok();
            let text = cols[11].trim();
            if text.is_empty() || number(10)? < TSV_MIN_CONFIDENCE {
                return None;
            }
            let part = |i: usize| cols[i].trim().parse::<u32>().ok();
            Some(TsvWord { text: text.to_string(), x: number(6)?, y: number(7)?, w: number(8)?, h: number(9)?, line: (part(2)?, part(3)?, part(4)?) })
        })
        .collect()
}

/// The text of `words`: the words of a line separated by spaces, lines by line breaks.
pub fn tsv_text(words: &[TsvWord]) -> String {
    let mut text = String::new();
    let mut last = None;
    for word in words {
        if let Some(line) = last {
            text.push(if line == word.line { ' ' } else { '\n' });
        }
        text.push_str(&word.text);
        last = Some(word.line);
    }
    text
}

#[cfg(test)]
mod tsv_tests {
    use super::*;

    const SAMPLE: &str = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
        1\t1\t0\t0\t0\t0\t0\t0\t800\t200\t-1\t\n\
        4\t1\t1\t1\t1\t0\t10\t10\t300\t20\t-1\t\n\
        5\t1\t1\t1\t1\t1\t10\t10\t60\t20\t96.5\tChưa\n\
        5\t1\t1\t1\t1\t2\t80\t10\t40\t20\t93.1\tbiết\n\
        5\t1\t1\t1\t1\t3\t130\t12\t9\t9\t0\t\"=~\n\
        4\t1\t1\t1\t2\t0\t10\t40\t300\t20\t-1\t\n\
        5\t1\t1\t1\t2\t1\t10\t40\t90\t20\t91\tnguyên\n\
        5\t1\t1\t1\t2\t2\t110\t40\t20\t20\t95\t \n\
        5\t1\t2\t1\t1\t1\t10\t90\t50\t20\t88\tgốc.\n\
        not a row at all\n";

    #[test]
    fn words_keep_their_text_box_and_line() {
        let words = tsv_words(SAMPLE);
        assert_eq!(words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), ["Chưa", "biết", "nguyên", "gốc."]);
        assert_eq!((words[1].x, words[1].y, words[1].w, words[1].h), (80.0, 10.0, 40.0, 20.0));
        assert_eq!(words[0].line, (1, 1, 1));
        assert_eq!(words[3].line, (2, 1, 1));
    }

    #[test]
    fn lines_are_joined_with_spaces_and_separated_by_line_breaks() {
        assert_eq!(tsv_text(&tsv_words(SAMPLE)), "Chưa biết\nnguyên\ngốc.");
        assert_eq!(tsv_text(&[]), "");
    }
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
