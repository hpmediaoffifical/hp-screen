// Zoom helpers for the edit page preview. The camera path itself comes from the core
// (zoom_track); these only read it and turn it into a CSS transform.
import type { Camera } from "../editApi.ts";

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
