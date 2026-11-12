// The drawn (smooth) cursor in the preview: the same samples and shapes the export uses
// (hp-core::cursor), read at a given source time.
import type { CursorSample, CursorShape } from "../editApi.ts";

/** The drawn cursor at `t` ms, interpolated between samples (as hp-core cursor::cursor_at). */
export function cursorSampleAt(samples: CursorSample[], t: number): CursorSample | null {
  if (samples.length === 0) return null;
  if (t <= samples[0].t_ms) return samples[0];
  const last = samples[samples.length - 1];
  if (t >= last.t_ms) return last;
  let lo = 0;
  let hi = samples.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (samples[mid].t_ms <= t) lo = mid + 1;
    else hi = mid;
  }
  const a = samples[lo - 1];
  const b = samples[lo];
  const f = (t - a.t_ms) / Math.max(1, b.t_ms - a.t_ms);
  const mix = (p: number, q: number) => p + (q - p) * f;
  return { t_ms: t, x: mix(a.x, b.x), y: mix(a.y, b.y), alpha: mix(a.alpha, b.alpha), press: f < 0.5 ? a.press : b.press };
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
