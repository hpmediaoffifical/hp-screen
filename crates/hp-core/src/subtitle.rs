//! Subtitles: reading whisper.cpp output, splitting it into readable cues, and writing SRT in
//! output time (after cuts).
use crate::cut::Cuts;
use serde::{Deserialize, Serialize};

pub const MAX_LINE_CHARS: usize = 42;
pub const MAX_LINES: usize = 2;
pub const MIN_CUE_MS: i64 = 1000;
pub const MAX_CUE_MS: i64 = 6000;
/// Burned-in subtitles: text height, bottom margin and widest box, as fractions of the frame.
pub const BURN_FONT: f32 = 0.045;
pub const BURN_BOTTOM: f32 = 0.06;
pub const BURN_MAX_WIDTH: f32 = 0.8;

/// Height of a burned-in cue of `lines` lines, as a fraction of the frame height.
pub fn burned_height(lines: usize) -> f32 {
    lines.max(1) as f32 * BURN_FONT * 1.3 + 2.0 * BURN_FONT * 0.3
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    pub start_ms: i64,
    pub end_ms: i64,
    /// One or two lines separated by '\n'.
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubtitleDoc {
    pub cues: Vec<Cue>,
    /// Draw the cues into the exported video.
    pub burn: bool,
}

/// One recognised segment, with token times when the recogniser gave them.
#[derive(Clone, Debug, PartialEq)]
pub struct AsrSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    /// (start ms, end ms, text) per token; a token starting with a space begins a word.
    pub tokens: Vec<(i64, i64, String)>,
}

/// Reads whisper.cpp's full JSON output (`-ojf`).
pub fn parse_whisper_json(json: &str) -> Result<Vec<AsrSegment>, String> {
    use serde_json::Value;
    let v: Value = serde_json::from_str(json).map_err(|e| format!("Kết quả nhận dạng không đọc được: {e}"))?;
    let items = v.get("transcription").and_then(Value::as_array).ok_or("Kết quả nhận dạng không có phần transcription")?;
    let ms = |o: &Value, k: &str| o.get("offsets").and_then(|x| x.get(k)).and_then(Value::as_i64).unwrap_or(0);
    let mut out = Vec::new();
    for it in items {
        let text = strip_special(it.get("text").and_then(Value::as_str).unwrap_or(""));
        let tokens = it
            .get("tokens")
            .and_then(Value::as_array)
            .map(|ts| {
                ts.iter()
                    .filter_map(|t| {
                        let s = t.get("text").and_then(Value::as_str)?;
                        (!s.trim_start().starts_with("[_")).then(|| (ms(t, "from"), ms(t, "to"), s.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.push(AsrSegment { start_ms: ms(it, "from"), end_ms: ms(it, "to"), text, tokens });
    }
    Ok(out)
}

/// Text without whisper's special markers such as `[_BEG_]` or `[_TT_125]`.
fn strip_special(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("[_") {
        out.push_str(&rest[..i]);
        match rest[i..].find(']') {
            Some(j) => rest = &rest[i + j + 1..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Words of a segment with the time each is said.
fn timed_words(s: &AsrSegment) -> Vec<(i64, i64, String)> {
    // From tokens: a token that starts with a space begins a new word.
    let mut words: Vec<(i64, i64, String)> = Vec::new();
    for (a, b, t) in &s.tokens {
        let starts_word = t.starts_with(' ') || words.is_empty();
        let piece = t.trim();
        if piece.is_empty() {
            continue;
        }
        match words.last_mut() {
            Some(w) if !starts_word => {
                w.1 = (*b).max(w.1);
                w.2.push_str(piece);
            }
            _ => words.push((*a, *b, piece.to_string())),
        }
    }
    let joined = words.iter().map(|w| w.2.as_str()).collect::<Vec<_>>().join(" ");
    if !words.is_empty() && joined == s.text {
        return words;
    }
    // No (usable) tokens: share the segment's time out by characters.
    let plain: Vec<&str> = s.text.split_whitespace().collect();
    let total: usize = plain.iter().map(|w| w.chars().count() + 1).sum::<usize>().max(1);
    let dur = (s.end_ms - s.start_ms).max(0) as f64;
    let mut done = 0usize;
    plain
        .iter()
        .map(|w| {
            let n = w.chars().count() + 1;
            let a = s.start_ms + (dur * done as f64 / total as f64).round() as i64;
            done += n;
            let b = s.start_ms + (dur * done as f64 / total as f64).round() as i64;
            (a, b, w.to_string())
        })
        .collect()
}

fn fits(text: &str) -> bool {
    let lines = wrap_lines(text);
    lines.len() <= MAX_LINES && lines.iter().all(|l| l.chars().count() <= MAX_LINE_CHARS)
}

fn ends_clause(word: &str) -> bool {
    word.ends_with(['.', ',', '!', '?', ';', ':', '…'])
}

/// Readable cues from recognised segments: at most two lines of 42 characters, 1–6 s each,
/// broken at punctuation when possible.
pub fn cues_from_segments(segments: &[AsrSegment]) -> Vec<Cue> {
    let mut cues: Vec<Cue> = Vec::new();
    for s in segments {
        let s = AsrSegment { text: strip_special(&s.text), ..s.clone() };
        if s.text.is_empty() {
            continue;
        }
        let words = timed_words(&s);
        let mut chunk: Vec<(i64, i64, String)> = Vec::new();
        let mut i = 0;
        while i < words.len() {
            let w = &words[i];
            let mut tried = chunk.clone();
            tried.push(w.clone());
            let text = tried.iter().map(|x| x.2.as_str()).collect::<Vec<_>>().join(" ");
            let too_long = !fits(&text) || tried.last().unwrap().1 - tried[0].0 > MAX_CUE_MS;
            if too_long && !chunk.is_empty() {
                // Break at the last clause end in the second half of the chunk, if any.
                let at = (chunk.len() / 2..chunk.len()).rev().find(|&k| ends_clause(&chunk[k].2)).map_or(chunk.len(), |k| k + 1);
                let rest: Vec<_> = chunk.split_off(at);
                cues.push(make_cue(&chunk));
                chunk = rest;
                continue; // retry the same word against the shorter chunk
            }
            chunk = tried;
            i += 1;
        }
        if !chunk.is_empty() {
            cues.push(make_cue(&chunk));
        }
        if let Some(last) = cues.last_mut() {
            last.end_ms = last.end_ms.max(s.end_ms);
        }
    }
    // At least a second on screen, but never over the next cue.
    for k in 0..cues.len() {
        let next = cues.get(k + 1).map(|c| c.start_ms);
        let c = &mut cues[k];
        let want = c.start_ms + MIN_CUE_MS;
        if c.end_ms < want {
            c.end_ms = next.map_or(want, |n| want.min(n)).max(c.end_ms);
        }
    }
    cues
}

fn make_cue(words: &[(i64, i64, String)]) -> Cue {
    let text = words.iter().map(|w| w.2.as_str()).collect::<Vec<_>>().join(" ");
    Cue { start_ms: words[0].0, end_ms: words[words.len() - 1].1, text: wrap_lines(&text).join("\n") }
}

/// `text` as lines of at most 42 characters, two when it fits, balanced.
pub fn wrap_lines(text: &str) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let len = |ws: &[&str]| ws.iter().map(|w| w.chars().count()).sum::<usize>() + ws.len().saturating_sub(1);
    if words.is_empty() {
        return Vec::new();
    }
    if len(&words) <= MAX_LINE_CHARS {
        return vec![words.join(" ")];
    }
    // Two lines, split where the longer line is shortest.
    let best = (1..words.len()).min_by_key(|&k| len(&words[..k]).max(len(&words[k..])));
    if let Some(k) = best
        && len(&words[..k]).max(len(&words[k..])) <= MAX_LINE_CHARS
    {
        return vec![words[..k].join(" "), words[k..].join(" ")];
    }
    // Too long for two: fill lines greedily.
    let mut lines: Vec<String> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    for w in words {
        let mut tried = cur.clone();
        tried.push(w);
        if len(&tried) > MAX_LINE_CHARS && !cur.is_empty() {
            lines.push(cur.join(" "));
            cur = vec![w];
        } else {
            cur = tried;
        }
    }
    if !cur.is_empty() {
        lines.push(cur.join(" "));
    }
    lines
}

/// The cue showing at source time `t_ms`.
pub fn cue_at(cues: &[Cue], t_ms: i64) -> Option<&Cue> {
    cues.iter().find(|c| c.start_ms <= t_ms && t_ms < c.end_ms)
}

/// SRT of `cues` in output time: cues inside removed spans are dropped, cues across a cut are
/// shortened to what is kept.
pub fn to_srt(cues: &[Cue], cuts: &Cuts) -> String {
    let stamp = |ms: i64| {
        let ms = ms.max(0);
        format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
    };
    let mut out = String::new();
    let mut n = 0;
    for c in cues {
        // The kept parts of the cue, in output time (contiguous: cuts are closed up).
        let mut before = 0i64;
        let mut span: Option<(i64, i64)> = None;
        for k in cuts.keep() {
            let (a, b) = (c.start_ms.max(k.start_ms), c.end_ms.min(k.end_ms));
            if a < b {
                let (oa, ob) = (before + a - k.start_ms, before + b - k.start_ms);
                span = Some(span.map_or((oa, ob), |(s, _)| (s, ob)));
            }
            before += k.end_ms - k.start_ms;
        }
        let text: Vec<&str> = c.text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let (Some((a, b)), false) = (span, text.is_empty()) else { continue };
        n += 1;
        out.push_str(&format!("{n}\n{} --> {}\n{}\n\n", stamp(a), stamp(b), text.join("\n").replace("-->", "->")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cut::Span;

    const SAMPLE: &str = r#"{
      "result": {"language": "vi"},
      "transcription": [
        {"timestamps": {"from": "00:00:00,000", "to": "00:00:02,500"}, "offsets": {"from": 0, "to": 2500},
         "text": " Xin chào các bạn.",
         "tokens": [
           {"text": "[_BEG_]", "offsets": {"from": 0, "to": 0}},
           {"text": " Xin", "offsets": {"from": 0, "to": 500}},
           {"text": " chào", "offsets": {"from": 500, "to": 1000}},
           {"text": " các", "offsets": {"from": 1000, "to": 1500}},
           {"text": " bạn", "offsets": {"from": 1500, "to": 2000}},
           {"text": ".", "offsets": {"from": 2000, "to": 2100}},
           {"text": "[_TT_125]", "offsets": {"from": 2500, "to": 2500}}
         ]}
      ]
    }"#;

    fn seg(start_ms: i64, end_ms: i64, text: &str) -> AsrSegment {
        AsrSegment { start_ms, end_ms, text: text.into(), tokens: vec![] }
    }

    #[test]
    fn whisper_json_is_read_without_special_tokens() {
        let s = parse_whisper_json(SAMPLE).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].start_ms, s[0].end_ms, s[0].text.as_str()), (0, 2500, "Xin chào các bạn."));
        assert_eq!(s[0].tokens.len(), 5);
        assert_eq!(s[0].tokens[1], (500, 1000, " chào".to_string()));
        assert!(parse_whisper_json("not json").is_err());
    }

    #[test]
    fn a_short_segment_is_one_cue() {
        let c = cues_from_segments(&parse_whisper_json(SAMPLE).unwrap());
        assert_eq!(c, vec![Cue { start_ms: 0, end_ms: 2500, text: "Xin chào các bạn.".into() }]);
    }

    #[test]
    fn long_segments_are_split_into_readable_cues() {
        let text = "Đầu tiên các bạn mở phần mềm lên, sau đó bấm vào nút Export ở góc trên bên phải. Tiếp theo chọn định dạng MP4 rồi bấm Lưu để hoàn tất việc xuất video.";
        let c = cues_from_segments(&[seg(0, 12_000, text)]);
        assert!(c.len() >= 2, "{c:?}");
        for cue in &c {
            assert!(cue.end_ms - cue.start_ms <= MAX_CUE_MS, "{cue:?}");
            let lines: Vec<&str> = cue.text.split('\n').collect();
            assert!(lines.len() <= MAX_LINES && lines.iter().all(|l| l.chars().count() <= MAX_LINE_CHARS), "{cue:?}");
        }
        assert_eq!(c.first().unwrap().start_ms, 0);
        let end = c.last().unwrap().end_ms;
        assert!((12_000..=13_000).contains(&end), "ends with the speech, held at most to a full second: {end}");
        let joined: Vec<String> = c.iter().map(|q| q.text.replace('\n', " ")).collect();
        assert_eq!(joined.join(" "), text, "no word lost");
        assert!(c.windows(2).all(|w| w[0].end_ms <= w[1].start_ms), "in order, no overlap");
    }

    #[test]
    fn a_break_prefers_punctuation() {
        let c = cues_from_segments(&[seg(0, 9000, "Mở phần mềm lên và chờ một chút, sau đó bấm vào nút Bắt đầu để quay màn hình nhé.")]);
        assert!(c[0].text.replace('\n', " ").ends_with(','), "{c:?}");
    }

    #[test]
    fn token_times_place_the_breaks() {
        let tokens: Vec<(i64, i64, String)> = (0..30).map(|i| (i * 300, i * 300 + 300, format!(" từ{i}"))).collect();
        let text: String = tokens.iter().map(|t| t.2.as_str()).collect::<String>().trim().to_string();
        let c = cues_from_segments(&[AsrSegment { start_ms: 0, end_ms: 9000, text, tokens }]);
        assert!(c.len() >= 2);
        let second = &c[1];
        let first_word = second.text.split_whitespace().next().unwrap();
        let i: i64 = first_word.trim_start_matches("từ").parse().unwrap();
        assert_eq!(second.start_ms, i * 300, "the cue starts when its first word is said");
    }

    #[test]
    fn very_short_cues_last_at_least_a_second() {
        let c = cues_from_segments(&[seg(0, 300, "Vâng."), seg(5000, 6000, "Được.")]);
        assert_eq!((c[0].start_ms, c[0].end_ms), (0, 1000));
        let close = cues_from_segments(&[seg(0, 300, "Vâng."), seg(600, 2000, "Được.")]);
        assert_eq!(close[0].end_ms, 600, "but never over the next one");
    }

    #[test]
    fn empty_segments_are_dropped() {
        assert!(cues_from_segments(&[seg(0, 1000, "   "), seg(1000, 2000, "[_TT_50]")]).len() <= 1);
        assert!(cues_from_segments(&[seg(0, 1000, "  ")]).is_empty());
    }

    #[test]
    fn lines_are_balanced_and_short() {
        assert_eq!(wrap_lines("Xin chào"), vec!["Xin chào"]);
        let l = wrap_lines("Bấm vào nút Export ở góc trên bên phải của cửa sổ chính");
        assert_eq!(l.len(), 2);
        assert!(l.iter().all(|x| x.chars().count() <= MAX_LINE_CHARS));
        assert!((l[0].chars().count() as i64 - l[1].chars().count() as i64).abs() <= 12, "{l:?}");
    }

    #[test]
    fn srt_uses_output_time() {
        let cues = vec![
            Cue { start_ms: 500, end_ms: 1500, text: "một".into() },
            Cue { start_ms: 3200, end_ms: 3800, text: "trong phần cắt".into() },
            Cue { start_ms: 4500, end_ms: 6000, text: "vắt qua\nhai dòng".into() },
            Cue { start_ms: 62_345, end_ms: 63_000, text: "phút".into() },
        ];
        let cuts = Cuts::new(70_000, 0, None, &[Span { start_ms: 3000, end_ms: 5000 }]);
        let srt = to_srt(&cues, &cuts);
        let expected = "1\n00:00:00,500 --> 00:00:01,500\nmột\n\n2\n00:00:03,000 --> 00:00:04,000\nvắt qua\nhai dòng\n\n3\n00:01:00,345 --> 00:01:01,000\nphút\n\n";
        assert_eq!(srt, expected);
    }

    #[test]
    fn srt_text_cannot_break_the_file() {
        let cues = vec![Cue { start_ms: 0, end_ms: 1000, text: "a\n\n\nb --> c".into() }];
        let srt = to_srt(&cues, &Cuts::new(5000, 0, None, &[]));
        assert_eq!(srt, "1\n00:00:00,000 --> 00:00:01,000\na\nb -> c\n\n");
    }

    #[test]
    fn old_edit_files_read_without_subtitles() {
        let doc = crate::edit::load_edit("{\"v\":1}").unwrap();
        assert!(doc.subtitles.cues.is_empty() && !doc.subtitles.burn);
    }
}
