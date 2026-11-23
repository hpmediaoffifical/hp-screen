/*! © 2026 HP Media (hpvn.media) — HP Screen. Bảo lưu mọi quyền. HP không có đại lý khác, không hợp tác với đơn vị nào, mọi sao chép đều là vi phạm. */
//! Finding sensitive information (phone numbers, emails, ID and card numbers) among the
//! words an OCR pass read, so one click can blur it all. Pure logic, tested in pii.test.ts.

/** One word as the core's OCR returns it: `text` plus its box in the picture's pixels. */
export interface OcrWord {
  text: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

/** A group of adjacent words that together hold one piece of sensitive information. */
export interface SensitiveGroup {
  words: OcrWord[];
  /** Union of the group's boxes. */
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Digits only, with the separators Vietnamese writing puts inside numbers removed. */
function digits(text: string): string {
  return text.replace(/[\s.,\-()]/g, "");
}

/**
 * What a run of digits is: a Vietnamese phone number, a 12-digit CCCD, a 9-digit old-style
 * ID, or a card number. Times, dates and file sizes do not fit these shapes.
 */
export function classifyDigits(run: string): "phone" | "id" | "card" | null {
  if (!/^\d+$/.test(run)) return null;
  const len = run.length;
  // +84 / 84 international form → the same number as the 0… one.
  if (run.startsWith("84") && (len === 11 || len === 12)) return "phone";
  if ((len === 10 || len === 11) && run.startsWith("0")) {
    const second = run[1];
    if (second >= "2" && second <= "9") return "phone";
  }
  if (len === 12) return "id"; // CCCD
  if (len === 9) return "id"; // CMND cũ
  if (len >= 13 && len <= 19) return "card"; // thẻ ngân hàng
  return null;
}

/** An email, possibly split across adjacent words ("phung" "@" "hpvn.media"). */
function isEmailRun(texts: string[]): boolean {
  // Email addresses have no spaces or diacritics, so every word must be email characters.
  if (!texts.every((t) => /^[A-Za-z0-9._%+-@]+$/.test(t))) return false;
  return /^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$/.test(texts.join(""));
}

/** Whether `a` and `b` sit on the same text line, one right after the other. */
function adjacent(a: OcrWord, b: OcrWord): boolean {
  const overlap =
    Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y) > Math.min(a.h, b.h) * 0.4;
  const gap = b.x - (a.x + a.w);
  return overlap && gap > -b.w * 0.6 && gap < Math.max(a.h, b.h) * 1.2;
}

/** The longest sensitive run at `words[i..]`, or null when the word starts nothing. */
function groupAt(words: OcrWord[], i: number): SensitiveGroup | null {
  for (const len of [4, 3, 2, 1]) {
    const run = words.slice(i, i + len);
    if (run.length < len) continue;
    if (run.slice(1).some((w, j) => !adjacent(run[j], w))) continue;
    const joined = run.map((w) => w.text).join("");
    const whole = digits(joined);
    // A phone written across words ("0901 234 567") only counts when the digits run together;
    // a lone short word of digits does not.
    const digitKind = len === 1 ? classifyDigits(whole) : whole.length >= 9 ? classifyDigits(whole) : null;
    if (digitKind || isEmailRun(run.map((w) => w.text))) {
      const x = Math.min(...run.map((w) => w.x));
      const y = Math.min(...run.map((w) => w.y));
      const right = Math.max(...run.map((w) => w.x + w.w));
      const bottom = Math.max(...run.map((w) => w.y + w.h));
      return { words: run, x, y, w: right - x, h: bottom - y };
    }
  }
  return null;
}

/** Groups of words that hold sensitive information, in reading order, longest match first. */
export function sensitiveGroups(words: OcrWord[]): SensitiveGroup[] {
  const out: SensitiveGroup[] = [];
  let i = 0;
  while (i < words.length) {
    const found = groupAt(words, i);
    if (found) {
      out.push(found);
      i += found.words.length;
    } else {
      i += 1;
    }
  }
  return out;
}
