// Zoom helpers for the edit page preview. The camera path itself comes from the core
// (zoom_track); these only read it and turn it into a CSS transform.
import type { Camera, ZoomKey, ZoomSegment } from "../editApi.ts";
import { sourceView, type View } from "./layoutMath.ts";

export const WHOLE: Camera = { cx: 0.5, cy: 0.5, scale: 1 };

/** The camera at `t` ms, interpolated between samples (same as hp-core zoom::camera_at). */
export function cameraAt(track: [number, Camera][], t: number): Camera {
  if (track.length === 0) return WHOLE;
  if (t <= track[0][0]) return track[0][1];
  const last = track[track.length - 1];
  if (t >= last[0]) return last[1];
  let lo = 0;
  let hi = track.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (track[mid][0] <= t) lo = mid + 1;
    else hi = mid;
  }
  const [t0, a] = track[lo - 1];
  const [t1, b] = track[lo];
  const f = (t - t0) / Math.max(1, t1 - t0);
  const mix = (x: number, y: number) => x + (y - x) * f;
  return { cx: mix(a.cx, b.cx), cy: mix(a.cy, b.cy), scale: mix(a.scale, b.scale) };
}

/** Translation (in unscaled pixels of a W×H element) and scale for `transform: scale(s) translate(-x, -y)`. */
export function viewOffset(cam: Camera, w: number, h: number): { s: number; x: number; y: number } {
  const s = Math.max(1, cam.scale);
  return { s, x: (cam.cx - 0.5 / s) * w, y: (cam.cy - 0.5 / s) * h };
}

/** The point of the frame (0–1) under a click at (px, py) on the zoomed W×H preview. */
export function pickPoint(cam: Camera, px: number, py: number, w: number, h: number): [number, number] {
  const s = Math.max(1, cam.scale);
  const clamp = (v: number) => Math.min(1, Math.max(0, v));
  return [clamp(cam.cx - 0.5 / s + px / w / s), clamp(cam.cy - 0.5 / s + py / h / s)];
}

// --- The zoom frame on the preview (spec "Zoom thông minh" 3) ---

export const MIN_SCALE = 1;
export const MAX_SCALE = 4;
/** Keyframes closer than this to another (or to the start) are refused. */
export const KEY_GAP_MS = 50;
/** A segment added with Z: length, zoom, and the least it may be. */
export const Z_LEN_MS = 3000;
export const Z_SCALE = 1.5;
export const Z_MIN_MS = 500;

/** The camera whose view (as `sourceView` gives it) is `view`. */
export function cameraFromView(view: View, srcW: number, srcH: number, fill: number | null): Camera {
  const [x, y, w, h] = view;
  const cropsWidth = !!fill && fill > 0 && srcW / srcH > fill;
  const scale = cropsWidth ? srcH / h : srcW / w;
  return { cx: (x + w / 2) / srcW, cy: (y + h / 2) / srcH, scale: Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale)) };
}

const inside = (v: View, srcW: number, srcH: number): View => {
  const [x, y, w, h] = v;
  return [Math.min(Math.max(0, x), Math.max(0, srcW - w)), Math.min(Math.max(0, y), Math.max(0, srcH - h)), w, h];
};

/** `cam` with its frame moved by (dx, dy) source pixels, kept inside the picture. */
export function moveFrame(cam: Camera, dx: number, dy: number, srcW: number, srcH: number, fill: number | null): Camera {
  const [x, y, w, h] = sourceView(cam, srcW, srcH, fill);
  return cameraFromView(inside([x + dx, y + dy, w, h], srcW, srcH), srcW, srcH, fill);
}

export type Corner = "nw" | "ne" | "sw" | "se";

/**
 * `cam` with its frame resized by dragging `corner` to (px, py) in source pixels: the opposite
 * corner stays when it can, the aspect is kept, the zoom stays within 1–4×, and the frame never
 * turns inside out.
 */
export function resizeFrame(cam: Camera, corner: Corner, px: number, py: number, srcW: number, srcH: number, fill: number | null): Camera {
  const [x, y, w, h] = sourceView(cam, srcW, srcH, fill);
  const [, , bw] = sourceView({ cx: 0.5, cy: 0.5, scale: 1 }, srcW, srcH, fill);
  const ratio = w / h;
  const east = corner === "ne" || corner === "se";
  const south = corner === "sw" || corner === "se";
  const ax = east ? x : x + w;
  const ay = south ? y : y + h;
  const dx = east ? px - ax : ax - px;
  const dy = south ? py - ay : ay - py;
  const nw = Math.min(bw / MIN_SCALE, Math.max(bw / MAX_SCALE, Math.max(dx, dy * ratio)));
  const nh = nw / ratio;
  const view: View = [east ? ax : ax - nw, south ? ay : ay - nh, nw, nh];
  return cameraFromView(inside(view, srcW, srcH), srcW, srcH, fill);
}

/** The keyframe being edited at `ms`: the last one at or before it, or -1 for the start frame. */
export function activeKey(seg: ZoomSegment, ms: number): number {
  let i = -1;
  seg.keys.forEach((k, j) => {
    if (k.t_ms <= ms) i = j;
  });
  return i;
}

const keyOf = (t_ms: number, cam: Camera): ZoomKey => ({ t_ms, cx: cam.cx, cy: cam.cy, scale: cam.scale });

/** `seg` with the frame active at `ms` set to `cam`; made manual. */
export function setActiveFrame(seg: ZoomSegment, ms: number, cam: Camera): ZoomSegment {
  const i = activeKey(seg, ms);
  if (i < 0) return { ...seg, center: [cam.cx, cam.cy], scale: cam.scale, manual: true };
  return { ...seg, keys: seg.keys.map((k, j) => (j === i ? keyOf(k.t_ms, cam) : k)), manual: true };
}

/** `seg` with a keyframe at `ms` showing `cam`, or null when `ms` is outside it or too close to another. */
export function addKey(seg: ZoomSegment, ms: number, cam: Camera): ZoomSegment | null {
  if (ms - seg.start_ms <= KEY_GAP_MS || ms >= seg.end_ms) return null;
  if (seg.keys.some((k) => Math.abs(k.t_ms - ms) <= KEY_GAP_MS)) return null;
  const keys = [...seg.keys, keyOf(Math.round(ms), cam)].sort((a, b) => a.t_ms - b.t_ms);
  // Keyframes need a fixed start frame: a cursor-following segment starts where the camera is.
  const center: [number, number] = seg.center ?? [cam.cx, cam.cy];
  return { ...seg, keys, center, manual: true };
}

/** `seg` moved by `dMs`, its keyframes with it. */
export function moveSegment(seg: ZoomSegment, dMs: number): ZoomSegment {
  return { ...seg, start_ms: seg.start_ms + dMs, end_ms: seg.end_ms + dMs, keys: seg.keys.map((k) => ({ ...k, t_ms: k.t_ms + dMs })) };
}

/** `seg` without the keyframes outside it (after an edge was dragged). */
export function keepKeysInside(seg: ZoomSegment): ZoomSegment {
  return { ...seg, keys: seg.keys.filter((k) => k.t_ms > seg.start_ms && k.t_ms < seg.end_ms) };
}

/** A new manual segment at `ms` (the Z key): 3 s, or up to the next segment or the end; null when under 0.5 s. */
export function addSegmentAt(segments: ZoomSegment[], ms: number, durationMs: number, center: [number, number] | null): { segments: ZoomSegment[]; index: number } | null {
  const start = Math.max(0, Math.round(ms));
  // Never on top of a segment that is already here.
  if (segments.some((s) => s.start_ms <= start && start < s.end_ms)) return null;
  const next = segments.filter((s) => s.start_ms > start).reduce((m, s) => Math.min(m, s.start_ms), Infinity);
  const end = Math.min(start + Z_LEN_MS, next, durationMs);
  if (end - start < Z_MIN_MS) return null;
  const seg: ZoomSegment = { start_ms: start, end_ms: end, scale: Z_SCALE, center: center ?? [0.5, 0.5], manual: true, keys: [], speed: "normal" };
  const out = [...segments, seg].sort((a, b) => a.start_ms - b.start_ms);
  return { segments: out, index: out.indexOf(seg) };
}

/** The frame shown for editing at `ms`: the active keyframe or start frame; `camera` places a cursor-following one. */
export function frameAt(seg: ZoomSegment, ms: number, camera: Camera): Camera {
  const i = activeKey(seg, ms);
  const raw = i >= 0 ? seg.keys[i] : seg.center ? { cx: seg.center[0], cy: seg.center[1], scale: seg.scale } : { cx: camera.cx, cy: camera.cy, scale: seg.scale };
  // Kept inside the picture exactly as the core keeps the camera (zoom::camera_track).
  const s = Math.min(MAX_SCALE, Math.max(MIN_SCALE, raw.scale));
  const half = 0.5 / s;
  const keep = (v: number) => Math.min(1 - half, Math.max(half, v));
  return { cx: keep(raw.cx), cy: keep(raw.cy), scale: s };
}

/** The scale slider: changes only the zoom of the active frame (a cursor-following start frame keeps following). */
export function setActiveScale(seg: ZoomSegment, ms: number, scale: number, camera: Camera): ZoomSegment {
  if (activeKey(seg, ms) < 0 && !seg.center) return { ...seg, scale, manual: true };
  return setActiveFrame(seg, ms, { ...frameAt(seg, ms, camera), scale });
}

/**
 * Where the frame sits on the editing picture, which shows the whole source: fractions of its
 * width and height. In Fill this is the cropped part the export shows.
 */
export function frameRect(cam: Camera, srcW: number, srcH: number, fill: number | null): { left: number; top: number; width: number; height: number } {
  const [x, y, w, h] = sourceView(cam, srcW, srcH, fill);
  return { left: x / srcW, top: y / srcH, width: w / srcW, height: h / srcH };
}
