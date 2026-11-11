// Stroke geometry and fading for the drawing overlay. Same rules and numbers as
// hp-core::stroke, so what the user sees on screen matches what goes into the video.
import type { Stroke } from "../liveApi.ts";

export const FADE_OUT_MS = 500;
export const COLORS: [number, number, number][] = [
  [0xef, 0x44, 0x44],
  [0xfa, 0xcc, 0x15],
  [0x22, 0xc5, 0x5e],
  [0x3b, 0x82, 0xf6],
  [0xff, 0xff, 0xff],
];
export const WIDTHS = [4, 8, 14];
const HEAD_ANGLE_DEG = 28;
const HEAD_MIN = 12;
const HEAD_PER_WIDTH = 4;

export type Point = [number, number];
export type Segment = [Point, Point];

/** The straight lines that draw `s`. */
export function segments(s: Stroke): Segment[] {
  const pts = s.points;
  if (pts.length === 0) return [];
  const first = pts[0];
  const last = pts[pts.length - 1];
  if (pts.length === 1) return [[first, first]];
  if (s.tool === "pen") return pts.slice(1).map((p, i) => [pts[i], p] as Segment);
  if (s.tool === "rect") {
    const a: Point = [first[0], last[1]];
    const b: Point = [last[0], first[1]];
    return [[first, b], [b, last], [last, a], [a, first]];
  }
  const out: Segment[] = [[first, last]];
  const dx = last[0] - first[0];
  const dy = last[1] - first[1];
  const len = Math.hypot(dx, dy);
  if (len > 0) {
    const [bx, by] = [-dx / len, -dy / len];
    const head = Math.max(HEAD_MIN, HEAD_PER_WIDTH * s.width);
    const t = (HEAD_ANGLE_DEG * Math.PI) / 180;
    for (const sign of [1, -1]) {
      const rx = bx * Math.cos(t) - by * Math.sin(t) * sign;
      const ry = bx * Math.sin(t) * sign + by * Math.cos(t);
      out.push([last, [last[0] + rx * head, last[1] + ry * head]]);
    }
  }
  return out;
}

/** Opacity of a stroke finished at `finishedAt` (null while drawing) at `now`, holding `holdMs` (0: forever). */
export function alphaAt(finishedAt: number | null, now: number, holdMs: number): number {
  if (finishedAt === null || holdMs <= 0) return 1;
  const past = now - finishedAt - holdMs;
  if (past <= 0) return 1;
  return Math.max(0, 1 - past / FADE_OUT_MS);
}
