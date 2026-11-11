// Editing subtitle cues on the edit page, and where they sit in the frame. The numbers match
// hp-core::subtitle and the export compositor.
import type { Cue } from "../editApi";

export const MAX_LINE_CHARS = 42;
export const BURN_FONT = 0.045;
export const BURN_BOTTOM = 0.06;
export const BURN_MAX_WIDTH = 0.8;
export const CHIP_BOTTOM = 0.08;
const MIN_LEN_MS = 100;

/** Height of a burned subtitle box with `lines` lines, as a fraction of the frame height. */
export const burnedHeight = (lines: number) => Math.max(1, lines) * BURN_FONT * 1.3 + 2 * BURN_FONT * 0.3;

/** Where the shortcut chip's bottom sits (fraction of frame height) with `subtitle` burned below it. */
export function chipBottom(subtitle: string | null): number {
  if (subtitle === null) return CHIP_BOTTOM;
  return BURN_BOTTOM + burnedHeight(subtitle.split("\n").length) + 0.02;
}

export function cueAt(cues: Cue[], ms: number): Cue | null {
  return cues.find((c) => c.start_ms <= ms && ms < c.end_ms && c.text.trim() !== "") ?? null;
}

const lenOf = (ws: string[]) => ws.reduce((n, w) => n + [...w].length, 0) + Math.max(0, ws.length - 1);

/** Like hp-core::subtitle::wrap_lines: one line up to 42 characters, else two balanced lines. */
export function wrapLines(text: string): string[] {
  const words = text.split(/\s+/).filter(Boolean);
  if (words.length === 0) return [];
  if (lenOf(words) <= MAX_LINE_CHARS) return [words.join(" ")];
  let best = 1;
  for (let k = 1; k < words.length; k++) {
    if (Math.max(lenOf(words.slice(0, k)), lenOf(words.slice(k))) < Math.max(lenOf(words.slice(0, best)), lenOf(words.slice(best)))) best = k;
  }
  if (Math.max(lenOf(words.slice(0, best)), lenOf(words.slice(best))) <= MAX_LINE_CHARS) {
    return [words.slice(0, best).join(" "), words.slice(best).join(" ")];
  }
  const lines: string[] = [];
  let cur: string[] = [];
  for (const w of words) {
    if (cur.length > 0 && lenOf([...cur, w]) > MAX_LINE_CHARS) {
      lines.push(cur.join(" "));
      cur = [w];
    } else cur.push(w);
  }
  if (cur.length > 0) lines.push(cur.join(" "));
  return lines;
}

const wrap = (text: string) => wrapLines(text).join("\n");

/** Joins cue `i` with the next one. */
export function mergeWithNext(cues: Cue[], i: number): Cue[] {
  const a = cues[i];
  const b = cues[i + 1];
  if (!a || !b) return cues;
  const merged = { start_ms: a.start_ms, end_ms: Math.max(a.end_ms, b.end_ms), text: wrap(`${a.text} ${b.text}`) };
  return [...cues.slice(0, i), merged, ...cues.slice(i + 2)];
}

/** Splits cue `i` at `ms`, sharing its words in proportion to the time on each side. */
export function splitCue(cues: Cue[], i: number, ms: number): Cue[] {
  const c = cues[i];
  if (!c || ms - c.start_ms < MIN_LEN_MS || c.end_ms - ms < MIN_LEN_MS) return cues;
  const words = c.text.split(/\s+/).filter(Boolean);
  if (words.length < 2) return cues;
  const k = Math.min(words.length - 1, Math.max(1, Math.round((words.length * (ms - c.start_ms)) / (c.end_ms - c.start_ms))));
  const first = { start_ms: c.start_ms, end_ms: ms, text: wrap(words.slice(0, k).join(" ")) };
  const second = { start_ms: ms, end_ms: c.end_ms, text: wrap(words.slice(k).join(" ")) };
  return [...cues.slice(0, i), first, second, ...cues.slice(i + 1)];
}

export const deleteCue = (cues: Cue[], i: number): Cue[] => cues.filter((_, j) => j !== i);

export const setCueText = (cues: Cue[], i: number, text: string): Cue[] => cues.map((c, j) => (j === i ? { ...c, text } : c));

/** New times for cue `i`, kept at or after 0 and at least 100 ms long. */
export function setCueTime(cues: Cue[], i: number, start: number, end: number): Cue[] {
  const s = Math.max(0, Math.round(start));
  const e = Math.max(s + MIN_LEN_MS, Math.round(end));
  return cues.map((c, j) => (j === i ? { ...c, start_ms: s, end_ms: e } : c));
}

/** Like hp-core::subtitle::burn_font_px: 4.5% of the height, smaller so 42 characters fit tall frames. */
export const burnFontPx = (w: number, h: number) => Math.max(10, Math.min(BURN_FONT * h, (BURN_MAX_WIDTH * w) / 24));

export interface SubtitleLayout {
  lines: string[];
  size: number;
  left: number;
  top: number;
  w: number;
  h: number;
}

/**
 * Lays `text` out as the export burns it: a box near the bottom of the frame, lines wrapped to
 * 80% of its width. `ctx`'s canvas is the whole output frame. Null for empty text.
 */
export function layoutSubtitle(ctx: CanvasRenderingContext2D, text: string): SubtitleLayout | null {
  const { width, height } = ctx.canvas;
  const size = burnFontPx(width, height);
  const padX = size * 0.6;
  const padY = size * 0.3;
  ctx.save();
  ctx.font = `600 ${size}px "Segoe UI", sans-serif`;
  const maxW = Math.max(10, BURN_MAX_WIDTH * width - 2 * padX);
  // Lines longer than the box are wrapped again at word boundaries, like DirectWrite does.
  const lines: string[] = [];
  for (const raw of text.split("\n")) {
    let cur = "";
    for (const w of raw.split(/\s+/).filter(Boolean)) {
      const next = cur ? `${cur} ${w}` : w;
      if (cur && ctx.measureText(next).width > maxW) {
        lines.push(cur);
        cur = w;
      } else cur = next;
    }
    if (cur) lines.push(cur);
  }
  const textW = lines.length ? Math.min(maxW, Math.max(...lines.map((l) => ctx.measureText(l).width))) : 0;
  ctx.restore();
  if (lines.length === 0) return null;
  const w = textW + 2 * padX;
  const h = lines.length * size * 1.3 + 2 * padY;
  return { lines, size, left: (width - w) / 2, top: height * (1 - BURN_BOTTOM) - h, w, h };
}

/** Where a chip's bottom goes (fraction of frame height) so it clears the laid-out subtitle. */
export const chipAbove = (l: SubtitleLayout, frameH: number) => Math.max(CHIP_BOTTOM, 1 - l.top / frameH + 0.02);

/** Paints a laid-out subtitle: dark rounded box, white centred lines. */
export function paintSubtitle(ctx: CanvasRenderingContext2D, l: SubtitleLayout): void {
  const { width } = ctx.canvas;
  ctx.save();
  ctx.font = `600 ${l.size}px "Segoe UI", sans-serif`;
  ctx.fillStyle = "rgba(0,0,0,.72)";
  ctx.beginPath();
  ctx.roundRect(l.left, l.top, l.w, l.h, l.size * 0.35);
  ctx.fill();
  ctx.fillStyle = "#fff";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  const lineH = l.size * 1.3;
  l.lines.forEach((line, n) => ctx.fillText(line, width / 2, l.top + l.size * 0.3 + lineH * (n + 0.5)));
  ctx.restore();
}
