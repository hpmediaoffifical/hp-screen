//! File names for recordings and screenshots.
use std::path::{Path, PathBuf};

pub const DEFAULT_PATTERN: &str = "hp_HH-mm-ss_dd-MM-yyyy";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// Expands `yyyy MM dd HH mm ss` (case-sensitive) and returns a valid Windows file stem.
pub fn render_pattern(pattern: &str, t: LocalTime) -> String {
    const TOKENS: [&str; 6] = ["yyyy", "MM", "dd", "HH", "mm", "ss"];
    let values = [
        format!("{:04}", t.year),
        format!("{:02}", t.month),
        format!("{:02}", t.day),
        format!("{:02}", t.hour),
        format!("{:02}", t.minute),
        format!("{:02}", t.second),
    ];
    let mut out = String::new();
    let mut rest = pattern;
    'scan: while let Some(c) = rest.chars().next() {
        for (token, value) in TOKENS.iter().zip(&values) {
            if let Some(after) = rest.strip_prefix(token) {
                out.push_str(value);
                rest = after;
                continue 'scan;
            }
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    sanitize(&out)
}

/// Makes `name` a valid Windows file stem; never returns an empty string.
pub fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim_start().trim_end_matches(['.', ' ']);
    if trimmed.is_empty() { "hp".to_string() } else { trimmed.to_string() }
}

/// First of `stem.ext`, `stem_2.ext`, `stem_3.ext`… in `dir` for which `exists` is false.
pub fn unique_path(dir: &Path, stem: &str, ext: &str, exists: &dyn Fn(&Path) -> bool) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !exists(&first) {
        return first;
    }
    (2u32..)
        .map(|n| dir.join(format!("{stem}_{n}.{ext}")))
        .find(|p| !exists(p))
        .expect("an unused suffix always exists")
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: LocalTime = LocalTime { year: 2026, month: 11, day: 11, hour: 14, minute: 30, second: 5 };

    #[test]
    fn default_pattern_puts_time_before_date() {
        assert_eq!(render_pattern(DEFAULT_PATTERN, T), "hp_14-30-05_11-11-2026");
    }

    #[test]
    fn month_and_minute_tokens_differ_by_case() {
        assert_eq!(render_pattern("yyyy-MM-dd_HH-mm-ss", T), "2026-11-11_14-30-05");
        let t = LocalTime { month: 3, minute: 7, ..T };
        assert_eq!(render_pattern("MM|mm", t), "03-07");
    }

    #[test]
    fn forbidden_characters_become_dashes() {
        assert_eq!(render_pattern("a:b/c*?", T), "a-b-c--");
        assert_eq!(sanitize(r#"a\b"c<d>e|f"#), "a-b-c-d-e-f");
    }

    #[test]
    fn empty_or_blank_pattern_falls_back_to_hp() {
        assert_eq!(render_pattern("", T), "hp");
        assert_eq!(render_pattern(" . ", T), "hp");
    }

    #[test]
    fn trailing_dots_and_spaces_are_trimmed() {
        assert_eq!(sanitize("ten. "), "ten");
    }

    #[test]
    fn unique_path_skips_taken_names() {
        let taken = |p: &Path| p.ends_with("hp_x.mp4") || p.ends_with("hp_x_2.mp4");
        assert_eq!(unique_path(Path::new("d"), "hp_x", "mp4", &taken), Path::new("d").join("hp_x_3.mp4"));
    }

    #[test]
    fn unique_path_keeps_free_name() {
        assert_eq!(unique_path(Path::new("d"), "hp_y", "png", &|_| false), Path::new("d").join("hp_y.png"));
    }
}
