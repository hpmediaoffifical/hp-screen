// Draws click and key effects on the preview canvas. The effects themselves (which rings,
// holds, wheel marks and chips exist, and when) come from the core; this only turns them into
// pixels for a given time, with the same rules and numbers as the export.
import type { EffectToggles, Effects, MouseButton } from "../editApi";

// Same values as hp-core::effects and src-tauri compositor.
const RIPPLE_MS = 480;
const RIPPLE_RADIUS = 0.025;
const RING_START = 0.3;
const DOT_RADIUS = 0.35;
const DOT_ALPHA = 0.55;
const WHEEL_FADE_MS = 150;
const CHIP_FADE_MS = 150;
const CHIP_BOTTOM = 0.08;
const CHIP_FONT = 0.032;
const COLOURS: Record<MouseButton, string> = { left: "#FF5A36", right: "#3B82F6", middle: "#22C55E" };

export function cursorAt(fx: Effects, t: number): [number, number] | null {
  const c = fx.cursor;
  if (c.length === 0) return null;
  if (t <= c[0][0]) return [c[0][1], c[0][2]];
  let lo = 0;
  let hi = c.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (c[mid][0] <= t) lo = mid + 1;
    else hi = mid;
  }
  const a = c[lo - 1];
  const b = c[lo];
  if (!b) return [a[1], a[2]];
  const f = b[0] > a[0] ? (t - a[0]) / (b[0] - a[0]) : 0;
  return [a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f];
}

const fade = (t: number, start: number, end: number, fadeIn: number, fadeOut: number) =>
  Math.min(1, Math.max(0, Math.min(fadeIn > 0 ? (t - start) / fadeIn : 1, fadeOut > 0 ? (end - t) / fadeOut : 1)));

/**
 * Draws the effects at source time `tMs` onto `ctx`, whose canvas shows the whole video:
 * `scale` is canvas pixels per source pixel, `videoH` the source height.
 */
export function drawEffects(ctx: CanvasRenderingContext2D, fx: Effects, tMs: number, on: EffectToggles, videoH: number, scale: number, chipBottom = CHIP_BOTTOM): void {
  const { width, height } = ctx.canvas;
  ctx.clearRect(0, 0, width, height);
  const r = RIPPLE_RADIUS * videoH * scale;
  if (on.clicks) {
    ctx.save();
    for (const h of fx.holds) {
      if (h.start_ms > tMs || tMs >= h.end_ms) continue;
      const p = cursorAt(fx, tMs);
      if (!p) continue;
      ctx.globalAlpha = DOT_ALPHA;
      ctx.fillStyle = COLOURS[h.button];
      ctx.beginPath();
      ctx.arc(p[0] * scale, p[1] * scale, DOT_RADIUS * r, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.lineWidth = Math.max(2, 0.12 * r);
    for (const rp of fx.ripples) {
      if (rp.t_ms > tMs || tMs >= rp.t_ms + RIPPLE_MS) continue;
      const p = (tMs - rp.t_ms) / RIPPLE_MS;
      const grow = 1 - (1 - p) * (1 - p);
      ctx.globalAlpha = 1 - p;
      ctx.strokeStyle = COLOURS[rp.button];
      ctx.beginPath();
      ctx.arc(rp.x * scale, rp.y * scale, r * (RING_START + (1 - RING_START) * grow), 0, Math.PI * 2);
      ctx.stroke();
    }
    for (const w of fx.wheels) {
      if (w.start_ms > tMs || tMs >= w.end_ms) continue;
      const p = cursorAt(fx, tMs);
      if (!p) continue;
      ctx.globalAlpha = fade(tMs, w.start_ms, w.end_ms, 0, WHEEL_FADE_MS);
      drawWheel(ctx, p[0] * scale, p[1] * scale, r, w.up);
    }
    ctx.restore();
  }
  if (on.keys) {
    const chip = fx.chips.find((c) => c.start_ms <= tMs && tMs < c.end_ms);
    if (chip) {
      ctx.save();
      ctx.globalAlpha = fade(tMs, chip.start_ms, chip.end_ms, CHIP_FADE_MS, CHIP_FADE_MS);
      drawChip(ctx, chip.text, Math.max(9, CHIP_FONT * height), chipBottom);
      ctx.restore();
    }
  }
}

function drawWheel(ctx: CanvasRenderingContext2D, x: number, y: number, r: number, up: boolean) {
  const w = 0.9 * r;
  const h = 1.3 * r;
  const left = x + r;
  const top = y + r;
  const line = Math.max(1, r * 0.08);
  ctx.fillStyle = "#fff";
  ctx.strokeStyle = "rgba(0,0,0,.8)";
  ctx.lineWidth = line;
  ctx.beginPath();
  ctx.roundRect(left, top, w, h, w / 2);
  ctx.fill();
  ctx.stroke();
  const cx = left + w / 2;
  ctx.lineWidth = line * 1.5;
  ctx.beginPath();
  ctx.moveTo(cx, top + h * 0.15);
  ctx.lineTo(cx, top + h * 0.4);
  ctx.stroke();
  const tip = up ? top - r * 0.55 : top + h + r * 0.55;
  const base = up ? top - r * 0.2 : top + h + r * 0.2;
  ctx.beginPath();
  ctx.moveTo(cx - w * 0.45, base);
  ctx.lineTo(cx, tip);
  ctx.lineTo(cx + w * 0.45, base);
  ctx.strokeStyle = "#fff";
  ctx.lineWidth = line * 2.5;
  ctx.stroke();
  ctx.strokeStyle = "rgba(0,0,0,.8)";
  ctx.lineWidth = line;
  ctx.stroke();
}

function drawChip(ctx: CanvasRenderingContext2D, text: string, size: number, bottom: number) {
  const { width, height } = ctx.canvas;
  ctx.font = `600 ${size}px "Segoe UI", sans-serif`;
  const m = ctx.measureText(text);
  const padX = size * 0.8;
  const padY = size * 0.3;
  const textH = size * 1.33;
  const w = m.width + 2 * padX;
  const h = textH + 2 * padY;
  const bottomY = height * (1 - bottom);
  const left = (width - w) / 2;
  ctx.fillStyle = "rgba(20,20,24,.85)";
  ctx.beginPath();
  ctx.roundRect(left, bottomY - h, w, h, h / 2);
  ctx.fill();
  ctx.fillStyle = "#fff";
  ctx.textBaseline = "middle";
  ctx.fillText(text, left + padX, bottomY - h / 2);
}
