//! Parsing and formatting of global hotkeys such as `Ctrl+Alt+R`.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hotkey {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    /// Windows virtual-key code.
    pub vk: u32,
}

const VK_F1: u32 = 0x70;
const VK_F12: u32 = 0x7B;
const VK_F24: u32 = 0x87;

/// Accepts A–Z, 0–9 and F1–F24 with modifiers, case-insensitive, spaces allowed.
/// Letters and digits need Ctrl, Alt or Win; F12 is refused because Windows reserves it.
pub fn parse_hotkey(s: &str) -> Result<Hotkey, String> {
    let mut hotkey = Hotkey { ctrl: false, alt: false, shift: false, win: false, vk: 0 };
    let mut has_key = false;
    for part in s.split('+') {
        let token = part.trim().to_ascii_lowercase();
        let modifier = match token.as_str() {
            "" => return Err("Thiếu phím trong tổ hợp".to_string()),
            "ctrl" | "control" => Some(&mut hotkey.ctrl),
            "alt" => Some(&mut hotkey.alt),
            "shift" => Some(&mut hotkey.shift),
            "win" | "super" | "meta" => Some(&mut hotkey.win),
            _ => None,
        };
        match modifier {
            Some(flag) if *flag => return Err(format!("Phím bổ trợ bị lặp: {}", part.trim())),
            Some(flag) => *flag = true,
            None if has_key => return Err("Tổ hợp chỉ được có một phím chính".to_string()),
            None => {
                hotkey.vk = key_code(&token).ok_or_else(|| format!("Phím không hỗ trợ: {}", part.trim()))?;
                has_key = true;
            }
        }
    }
    if !has_key {
        return Err("Tổ hợp chưa có phím chính".to_string());
    }
    if hotkey.vk == VK_F12 {
        return Err("F12 được Windows dành riêng, hãy chọn phím khác".to_string());
    }
    let is_function_key = (VK_F1..=VK_F24).contains(&hotkey.vk);
    if !is_function_key && !(hotkey.ctrl || hotkey.alt || hotkey.win) {
        return Err("Phím chữ và số cần đi kèm Ctrl, Alt hoặc Win".to_string());
    }
    Ok(hotkey)
}

fn key_code(token: &str) -> Option<u32> {
    let mut chars = token.chars();
    match (chars.next(), chars.as_str()) {
        (Some(c @ ('a'..='z' | '0'..='9')), "") => Some(c.to_ascii_uppercase() as u32),
        (Some('f'), number) if !number.starts_with('0') => {
            let n: u32 = number.parse().ok()?;
            (1..=24).contains(&n).then(|| VK_F1 + n - 1)
        }
        _ => None,
    }
}

/// Canonical form: modifiers in the order Ctrl, Alt, Shift, Win, then the key.
pub fn format_hotkey(h: &Hotkey) -> String {
    let mut out = String::new();
    for (on, name) in [(h.ctrl, "Ctrl+"), (h.alt, "Alt+"), (h.shift, "Shift+"), (h.win, "Win+")] {
        if on {
            out.push_str(name);
        }
    }
    if (VK_F1..=VK_F24).contains(&h.vk) {
        out.push_str(&format!("F{}", h.vk - VK_F1 + 1));
    } else if let Some(c) = char::from_u32(h.vk) {
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_toggle_hotkey() {
        let h = parse_hotkey("Ctrl+Alt+R").unwrap();
        assert_eq!(h, Hotkey { ctrl: true, alt: true, shift: false, win: false, vk: 0x52 });
    }

    #[test]
    fn parsing_ignores_case_order_and_spaces() {
        assert_eq!(format_hotkey(&parse_hotkey("alt + ctrl + r").unwrap()), "Ctrl+Alt+R");
        assert_eq!(format_hotkey(&parse_hotkey("win+shift+5").unwrap()), "Shift+Win+5");
    }

    #[test]
    fn letters_need_ctrl_alt_or_win() {
        assert!(parse_hotkey("R").is_err());
        assert!(parse_hotkey("Shift+R").is_err());
    }

    #[test]
    fn function_keys_work_alone_except_f12() {
        assert_eq!(parse_hotkey("F9").unwrap().vk, 0x78);
        assert_eq!(format_hotkey(&parse_hotkey("ctrl+f24").unwrap()), "Ctrl+F24");
        assert!(parse_hotkey("F12").is_err());
        assert!(parse_hotkey("Ctrl+F12").is_err());
        assert!(parse_hotkey("F25").is_err());
        assert!(parse_hotkey("F0").is_err());
    }

    #[test]
    fn rejects_missing_duplicate_or_unknown_keys() {
        assert!(parse_hotkey("Ctrl+Alt").is_err());
        assert!(parse_hotkey("Ctrl+A+B").is_err());
        assert!(parse_hotkey("Ctrl+Ctrl+A").is_err());
        assert!(parse_hotkey("Ctrl+?").is_err());
        assert!(parse_hotkey("").is_err());
        assert!(parse_hotkey("Ctrl++A").is_err());
    }
}
