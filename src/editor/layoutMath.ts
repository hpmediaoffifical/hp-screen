// Frame layout helpers for the preview; the same rules as hp-core::layout::source_view.
import type { Camera } from "../editApi.ts";

export type View = [number, number, number, number];

/** The part of the source shown (x, y, w, h in source pixels): zoom view, cropped to `fillAspect`. */
export function sourceView(cam: Camera, srcW: number, srcH: number, fillAspect: number | null): View {
  const s = Math.max(1, cam.scale);
  const w = srcW / s;
  const h = srcH / s;
  const x = cam.cx * srcW - w / 2;
  const y = cam.cy * srcH - h / 2;
  if (!fillAspect || !(fillAspect > 0)) return [x, y, w, h];
  const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), Math.max(lo, hi));
  if (w / h > fillAspect) {
    const nw = h * fillAspect;
    return [clamp(cam.cx * srcW - nw / 2, 0, srcW - nw), y, nw, h];
  }
  const nh = w / fillAspect;
  return [x, clamp(cam.cy * srcH - nh / 2, 0, srcH - nh), w, nh];
}

/**
 * CSS angle (deg) of a linear gradient from the top-left to the bottom-right corner of a w×h
 * box, matching Direct2D's (0,0)→(w,h) gradient ("to bottom right" in CSS is a different line).
 */
export function gradientAngle(w: number, h: number): number {
  return 180 - (Math.atan(w / Math.max(1e-9, h)) * 180) / Math.PI;
}

/** The frame point (0–1) under a click at (px, py) on a window of ww×wh showing `view`. */
export function pickInView(view: View, px: number, py: number, ww: number, wh: number, srcW: number, srcH: number): [number, number] {
  const clamp = (v: number) => Math.min(1, Math.max(0, v));
  const [vx, vy, vw, vh] = view;
  return [clamp((vx + (px / ww) * vw) / srcW), clamp((vy + (py / wh) * vh) / srcH)];
}
