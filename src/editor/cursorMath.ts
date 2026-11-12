// The drawn (smooth) cursor in the preview: the same samples and shapes the export uses
// (hp-core::cursor), read at a given source time.
import type { CursorSample, CursorShape } from "../editApi.ts";

/** The drawn cursor at `t` ms, interpolated between samples (as hp-core cursor::cursor_at). */
export interface Cursor {
  x: number;
  y: number;
  alpha: number;
  press: number;
}

export function cursorSampleAt(samples: CursorSample[], t: number): Cursor | null {
  if (samples.length === 0) return null;
  const at = (s: CursorSample): Cursor => ({ x: s[1], y: s[2], alpha: s[3], press: s[4] });
  if (t <= samples[0][0]) return at(samples[0]);
  const last = samples[samples.length - 1];
  if (t >= last[0]) return at(last);
  let lo = 0;
  let hi = samples.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (samples[mid][0] <= t) lo = mid + 1;
    else hi = mid;
  }
  const a = samples[lo - 1];
  const b = samples[lo];
  const f = (t - a[0]) / Math.max(1, b[0] - a[0]);
  const mix = (p: number, q: number) => p + (q - p) * f;
  return { x: mix(a[1], b[1]), y: mix(a[2], b[2]), alpha: mix(a[3], b[3]), press: f < 0.5 ? a[4] : b[4] };
}

/** The cursor's shape id at `t` ms: the last change at or before it, else the first shape; 0 is hidden. */
export function cursorShapeAt(fx: { cursor_ids: [number, number][]; shapes: CursorShape[] }, t: number): number {
  let id: number | null = null;
  for (const [at, shape] of fx.cursor_ids) {
    if (at > t) break;
    id = shape;
  }
  return id ?? fx.shapes[0]?.id ?? 0;
}
