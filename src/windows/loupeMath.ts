/*! © 2026 HP Media (hpvn.media) — HP Screen. Bảo lưu mọi quyền. HP không có đại lý khác, không hợp tác với đơn vị nào, mọi sao chép đều là vi phạm. */
// Where the magnifier looks and where it sits. Kept apart from the component so it can be tested.

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

/** Screen pixels shown across the magnifier (odd, so one pixel is the middle). */
export const LOUPE_CELLS = 23;
/** CSS pixels each screen pixel is drawn at. */
export const LOUPE_ZOOM = 6;
/** Distance kept between the point and the magnifier, CSS pixels. */
const GAP = 18;
const EDGE = 4;

/**
 * The square of the picture the magnifier shows for the point `at`: `x`, `y` is its top-left
 * pixel, `cx`, `cy` the pixel in its middle. A point on the picture's far edge (x = width, as
 * the end of an area can be) looks at the last pixel.
 */
export function loupeSource(at: Point, width: number, height: number): { x: number; y: number; cx: number; cy: number } {
  const half = (LOUPE_CELLS - 1) / 2;
  const cx = Math.min(Math.max(Math.round(at.x), 0), Math.max(0, width - 1));
  const cy = Math.min(Math.max(Math.round(at.y), 0), Math.max(0, height - 1));
  return { x: cx - half, y: cy - half, cx, cy };
}

/**
 * Top-left corner of a `w` × `h` magnifier for the point `at` (all CSS pixels). It sits
 * diagonally off the point, on the side away from `away` (the middle of the area being
 * shaped), so it never covers the area's inside; near a screen edge it flips to the other side.
 */
export function loupePlace(at: Point, away: Point | null, w: number, h: number, vw: number, vh: number): Point {
  const side = (p: number, from: number | undefined, size: number, limit: number) => {
    const after = p + GAP;
    const before = p - GAP - size;
    // Away from the area's middle; with no area (or dead level with it), after the point.
    const preferAfter = from === undefined || p >= from;
    const fits = (v: number) => v >= EDGE && v + size <= limit - EDGE;
    const first = preferAfter ? after : before;
    const second = preferAfter ? before : after;
    if (fits(first)) return first;
    if (fits(second)) return second;
    return Math.min(Math.max(first, EDGE), Math.max(EDGE, limit - EDGE - size));
  };
  return { x: side(at.x, away?.x, w, vw), y: side(at.y, away?.y, h, vh) };
}

type Handle = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";

/**
 * The point of `box` a resize by `handle` is moving: the corner itself, or for an edge the
 * place on it level with the pointer.
 */
export function handlePoint(box: Box, handle: Handle, pointer: Point): Point {
  const within = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);
  const x = handle.includes("w") ? box.x : handle.includes("e") ? box.x + box.w : within(pointer.x, box.x, box.x + box.w);
  const y = handle.includes("n") ? box.y : handle.includes("s") ? box.y + box.h : within(pointer.y, box.y, box.y + box.h);
  return { x, y };
}
