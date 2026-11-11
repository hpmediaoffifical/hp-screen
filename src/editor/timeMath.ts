// Small time helpers for the edit page, kept free of Tauri imports so they can be tested
// with `node --test` (see package.json "test").
import type { Span } from "../editApi.ts";

/**
 * Where playback should be for source time `ms`: `ms` itself when it is kept, else the start
 * of the next kept span, or `null` past the last one.
 */
export function keptTime(keep: Span[], ms: number): number | null {
  for (const k of keep) {
    if (ms < k.start_ms) return k.start_ms;
    if (ms < k.end_ms) return ms;
  }
  return null;
}

/** Where pressing play at `ms` starts: there if it is kept, else the next kept part; from the start again at the end. */
export function playFrom(keep: Span[], ms: number): number {
  const first = keep[0];
  const last = keep[keep.length - 1];
  if (!first || !last) return 0;
  // Playback parks 1 ms before the end; pressing play there means "again".
  if (ms >= last.end_ms - 1) return first.start_ms;
  return keptTime(keep, ms) ?? first.start_ms;
}

/** Adds a removed span to `cuts`, merging overlapping and touching spans, sorted. */
export function addCut(cuts: Span[], span: Span): Span[] {
  const all = [...cuts, span].filter((c) => c.end_ms > c.start_ms).sort((a, b) => a.start_ms - b.start_ms);
  const out: Span[] = [];
  for (const c of all) {
    const prev = out[out.length - 1];
    if (prev && c.start_ms <= prev.end_ms) prev.end_ms = Math.max(prev.end_ms, c.end_ms);
    else out.push({ ...c });
  }
  return out;
}
