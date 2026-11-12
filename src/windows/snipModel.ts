// Pure logic of the capture-and-annotate overlay: the annotation shapes, their numbering,
// picking and moving them, and the undo/redo history. No DOM here, so it can be tested with
// `node --test tests/`. All coordinates are image (physical) pixels.

export interface Point {
  x: number;
  y: number;
}
export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Box behind text so it stands out. */
export type TextBg = "none" | "dark" | "light" | "orange" | "color";

export type Shape =
  | { kind: "pen" | "highlight"; color: string; width: number; points: Point[] }
  | { kind: "rect" | "ellipse" | "arrow" | "blockArrow" | "line"; color: string; width: number; from: Point; to: Point }
  | { kind: "mosaic"; from: Point; to: Point }
  | { kind: "text"; color: string; size: number; at: Point; text: string; bg: TextBg }
  /** A numbered circle; `size` is its radius. Its number comes from its place in the list. */
  | { kind: "step"; color: string; size: number; at: Point };

export type TextShape = Extract<Shape, { kind: "text" }>;
/** Measures a text annotation (needs a canvas, so the overlay provides it). */
export type MeasureText = (s: TextShape) => Box;

export const normalize = (a: Point, b: Point): Box => ({
  x: Math.min(a.x, b.x),
  y: Math.min(a.y, b.y),
  w: Math.abs(b.x - a.x),
  h: Math.abs(b.y - a.y),
});

// ---- step numbers ----

/** The number shown on each shape: 1, 2, 3… for steps in the order placed, 0 for other shapes. */
export function stepNumbers(shapes: Shape[]): number[] {
  let n = 0;
  return shapes.map((s) => (s.kind === "step" ? ++n : 0));
}

/** The number the next step will get. */
export const nextStepNumber = (shapes: Shape[]): number => shapes.filter((s) => s.kind === "step").length + 1;

// ---- history ----

export interface History {
  past: Shape[][];
  present: Shape[];
  future: Shape[][];
}
const HISTORY_LIMIT = 200;

export const historyOf = (present: Shape[]): History => ({ past: [], present, future: [] });

/** A new change: the current shapes go on the undo list and the redo list is dropped. */
export const commit = (h: History, next: Shape[]): History => ({
  past: [...h.past, h.present].slice(-HISTORY_LIMIT),
  present: next,
  future: [],
});

export const undo = (h: History): History =>
  h.past.length === 0 ? h : { past: h.past.slice(0, -1), present: h.past[h.past.length - 1], future: [h.present, ...h.future] };

export const redo = (h: History): History =>
  h.future.length === 0 ? h : { past: [...h.past, h.present], present: h.future[0], future: h.future.slice(1) };

// ---- geometry ----

const distanceToSegment = (p: Point, a: Point, b: Point): number => {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const length2 = dx * dx + dy * dy;
  const t = length2 === 0 ? 0 : Math.min(1, Math.max(0, ((p.x - a.x) * dx + (p.y - a.y) * dy) / length2));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
};

const insideBox = (p: Point, b: Box, pad = 0) =>
  p.x >= b.x - pad && p.x <= b.x + b.w + pad && p.y >= b.y - pad && p.y <= b.y + b.h + pad;

/** Outline of a block arrow ("fat" arrow) from `from` to `to`; empty when it is too short to draw. */
export function blockArrowOutline(from: Point, to: Point, width: number): Point[] {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const length = Math.hypot(dx, dy);
  if (length < 2) return [];
  // Its thickness grows with its length.
  const shaftHalf = Math.max(length * 0.11, width * 2.5);
  const headHalf = shaftHalf * 2.1;
  const headLength = Math.min(length * 0.45, headHalf * 1.3);
  const neck = length - headLength;
  const cos = dx / length;
  const sin = dy / length;
  const local: [number, number][] = [
    [0, -shaftHalf],
    [neck, -shaftHalf],
    [neck, -headHalf],
    [length, 0],
    [neck, headHalf],
    [neck, shaftHalf],
    [0, shaftHalf],
  ];
  return local.map(([u, v]) => ({ x: from.x + u * cos - v * sin, y: from.y + u * sin + v * cos }));
}

const insidePolygon = (p: Point, polygon: Point[]): boolean => {
  let inside = false;
  for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
    const a = polygon[i];
    const b = polygon[j];
    if (a.y > p.y !== b.y > p.y && p.x < ((b.x - a.x) * (p.y - a.y)) / (b.y - a.y) + a.x) inside = !inside;
  }
  return inside;
};

/** Area a shape covers on the picture, stroke included. */
export function shapeBounds(s: Shape, measure: MeasureText): Box {
  const grow = (b: Box, by: number): Box => ({ x: b.x - by, y: b.y - by, w: b.w + by * 2, h: b.h + by * 2 });
  const around = (points: Point[]): Box => {
    const xs = points.map((p) => p.x);
    const ys = points.map((p) => p.y);
    const x = Math.min(...xs);
    const y = Math.min(...ys);
    return { x, y, w: Math.max(...xs) - x, h: Math.max(...ys) - y };
  };
  switch (s.kind) {
    case "pen":
    case "highlight":
      return grow(around(s.points), s.width / 2);
    case "rect":
    case "ellipse":
    case "line":
      return grow(normalize(s.from, s.to), s.width / 2);
    case "arrow":
      return grow(normalize(s.from, s.to), Math.max(12, s.width * 4) / 2);
    case "blockArrow": {
      const outline = blockArrowOutline(s.from, s.to, s.width);
      return grow(outline.length ? around(outline) : normalize(s.from, s.to), s.width / 2);
    }
    case "mosaic":
      return normalize(s.from, s.to);
    case "text":
      return measure(s);
    case "step":
      return { x: s.at.x - s.size, y: s.at.y - s.size, w: s.size * 2, h: s.size * 2 };
  }
}

/**
 * Whether a press at `p` picks shape `s`. Outlines (rectangle, ellipse, lines, strokes) are
 * picked near the line itself, within `tolerance` pixels; filled things (block arrow, mosaic,
 * text, steps) anywhere inside.
 */
export function hitShape(s: Shape, p: Point, tolerance: number, measure: MeasureText): boolean {
  switch (s.kind) {
    case "pen":
    case "highlight": {
      const reach = s.width / 2 + tolerance;
      if (s.points.length === 1) return Math.hypot(p.x - s.points[0].x, p.y - s.points[0].y) <= reach;
      return s.points.some((a, i) => i > 0 && distanceToSegment(p, s.points[i - 1], a) <= reach);
    }
    case "line":
    case "arrow":
      return distanceToSegment(p, s.from, s.to) <= s.width / 2 + tolerance;
    case "rect": {
      const b = normalize(s.from, s.to);
      const reach = s.width / 2 + tolerance;
      if (!insideBox(p, b, reach)) return false;
      const fromEdge = Math.min(p.x - b.x, b.x + b.w - p.x, p.y - b.y, b.y + b.h - p.y);
      return fromEdge <= reach;
    }
    case "ellipse": {
      const b = normalize(s.from, s.to);
      const rx = b.w / 2;
      const ry = b.h / 2;
      const reach = s.width / 2 + tolerance;
      if (rx < reach || ry < reach) return insideBox(p, b, reach);
      const r = Math.hypot((p.x - b.x - rx) / rx, (p.y - b.y - ry) / ry);
      return Math.abs(r - 1) * Math.min(rx, ry) <= reach;
    }
    case "blockArrow": {
      const outline = blockArrowOutline(s.from, s.to, s.width);
      if (outline.length === 0) return false;
      if (insidePolygon(p, outline)) return true;
      return outline.some((a, i) => distanceToSegment(p, a, outline[(i + 1) % outline.length]) <= s.width / 2 + tolerance);
    }
    case "mosaic":
      return insideBox(p, normalize(s.from, s.to), tolerance);
    case "text":
      return insideBox(p, measure(s), tolerance);
    case "step":
      return Math.hypot(p.x - s.at.x, p.y - s.at.y) <= s.size + tolerance;
  }
}

/** Index of the top-most shape a press at `p` picks, or -1. `only` limits it to one kind. */
export function shapeAt(shapes: Shape[], p: Point, tolerance: number, measure: MeasureText, only?: Shape["kind"]): number {
  for (let i = shapes.length - 1; i >= 0; i--) {
    if (only && shapes[i].kind !== only) continue;
    if (hitShape(shapes[i], p, tolerance, measure)) return i;
  }
  return -1;
}

/** The same shape shifted by (`dx`, `dy`). */
export function moveShape(s: Shape, dx: number, dy: number): Shape {
  const by = (p: Point): Point => ({ x: p.x + dx, y: p.y + dy });
  switch (s.kind) {
    case "pen":
    case "highlight":
      return { ...s, points: s.points.map(by) };
    case "text":
    case "step":
      return { ...s, at: by(s.at) };
    default:
      return { ...s, from: by(s.from), to: by(s.to) };
  }
}
