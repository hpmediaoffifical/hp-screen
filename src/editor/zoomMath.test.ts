import assert from "node:assert/strict";
import { test } from "node:test";
import { cameraAt, pickPoint, viewOffset } from "./zoomMath.ts";

const c = (cx: number, cy: number, scale: number) => ({ cx, cy, scale });

test("camera is interpolated and held at the ends", () => {
  const track: [number, ReturnType<typeof c>][] = [
    [0, c(0.5, 0.5, 1)],
    [100, c(0.5, 0.5, 2)],
  ];
  assert.equal(cameraAt(track, 50).scale, 1.5);
  assert.equal(cameraAt(track, -5).scale, 1);
  assert.equal(cameraAt(track, 900).scale, 2);
  assert.deepEqual(cameraAt([], 10), c(0.5, 0.5, 1));
});

test("view offset of a 2x zoom on the top-left quarter", () => {
  assert.deepEqual(viewOffset(c(0.25, 0.25, 2), 800, 400), { s: 2, x: 0, y: 0 });
  assert.deepEqual(viewOffset(c(0.5, 0.5, 2), 800, 400), { s: 2, x: 200, y: 100 });
  assert.deepEqual(viewOffset(c(0.5, 0.5, 1), 800, 400), { s: 1, x: 0, y: 0 });
});

test("a click on the zoomed preview picks the frame point under it", () => {
  assert.deepEqual(pickPoint(c(0.5, 0.5, 1), 200, 100, 800, 400), [0.25, 0.25]);
  assert.deepEqual(pickPoint(c(0.5, 0.5, 2), 400, 200, 800, 400), [0.5, 0.5]);
  assert.deepEqual(pickPoint(c(0.25, 0.25, 2), 800, 400, 800, 400), [0.5, 0.5]);
});

// --- Zoom frame and keyframes ---
import { activeKey, addKey, addSegmentAt, cameraFromView, keepKeysInside, moveFrame, moveSegment, resizeFrame, setActiveFrame } from "./zoomMath.ts";
import { sourceView } from "./layoutMath.ts";
import type { ZoomSegment } from "../editApi.ts";

const near = (a: number, b: number, tol = 1e-6) => Math.abs(a - b) <= tol;
const seg = (over: Partial<ZoomSegment> = {}): ZoomSegment => ({ start_ms: 1000, end_ms: 5000, scale: 2, center: [0.5, 0.5], manual: false, keys: [], speed: "normal", ...over });

test("frameRoundTripsInFillMode", () => {
  for (const fill of [null, 9 / 16, 3]) {
    const cam = c(0.4, 0.6, 2);
    const back = cameraFromView(sourceView(cam, 1600, 900, fill), 1600, 900, fill);
    assert.ok(near(back.cx, cam.cx) && near(back.cy, cam.cy) && near(back.scale, cam.scale), `${fill}: ${JSON.stringify(back)}`);
  }
});

test("moveFrame stays inside the picture", () => {
  const moved = moveFrame(c(0.5, 0.5, 2), 10_000, -10_000, 1000, 500, null);
  const [x, y, w] = sourceView(moved, 1000, 500, null);
  assert.ok(near(x + w, 1000, 1e-3) && near(y, 0, 1e-3), JSON.stringify(moved));
  assert.ok(near(moved.scale, 2));
});

test("resizeIsClampedAndNeverFlips", () => {
  const tiny = resizeFrame(c(0.5, 0.5, 2), "se", 0, 0, 1000, 500, null);
  assert.ok(near(tiny.scale, 4, 1e-6), `dragged past the opposite corner: ${tiny.scale}`);
  const huge = resizeFrame(c(0.5, 0.5, 2), "se", 5000, 5000, 1000, 500, null);
  assert.ok(near(huge.scale, 1, 1e-6), `dragged out of the picture: ${huge.scale}`);
  const [x, y, w, h] = sourceView(huge, 1000, 500, null);
  assert.ok(x >= -1e-6 && y >= -1e-6 && x + w <= 1000 + 1e-6 && y + h <= 500 + 1e-6);
});

test("resize keeps the opposite corner", () => {
  const out = resizeFrame(c(0.5, 0.5, 2), "se", 800, 400, 1000, 500, null);
  const [x, y, w] = sourceView(out, 1000, 500, null);
  assert.ok(near(x, 250, 1) && near(y, 125, 1) && near(w, 550, 1), JSON.stringify([x, y, w]));
  const nw = resizeFrame(c(0.5, 0.5, 2), "nw", 150, 75, 1000, 500, null);
  const v = sourceView(nw, 1000, 500, null);
  assert.ok(near(v[0] + v[2], 750, 1) && near(v[1] + v[3], 375, 1), "the se corner stays");
});

test("activeKey picks the last key at or before", () => {
  const s = seg({ keys: [{ t_ms: 2000, cx: 0.2, cy: 0.2, scale: 2 }, { t_ms: 3000, cx: 0.8, cy: 0.8, scale: 3 }] });
  assert.equal(activeKey(s, 1500), -1);
  assert.equal(activeKey(s, 2000), 0);
  assert.equal(activeKey(s, 4999), 1);
});

test("setActiveFrame edits the active key and makes the segment manual", () => {
  const s = seg({ keys: [{ t_ms: 2000, cx: 0.2, cy: 0.2, scale: 2 }] });
  const a = setActiveFrame(s, 2500, c(0.3, 0.4, 3));
  assert.deepEqual(a.keys[0], { t_ms: 2000, cx: 0.3, cy: 0.4, scale: 3 });
  assert.ok(a.manual);
  const b = setActiveFrame(seg({ center: null }), 1200, c(0.3, 0.4, 3));
  assert.deepEqual([b.center, b.scale], [[0.3, 0.4], 3], "the main frame; a cursor-following segment gets a fixed centre");
});

test("addKey refuses near another key", () => {
  const s = seg({ keys: [{ t_ms: 2000, cx: 0.2, cy: 0.2, scale: 2 }] });
  assert.equal(addKey(s, 2040, c(0.5, 0.5, 2)), null);
  assert.equal(addKey(s, 1030, c(0.5, 0.5, 2)), null, "too close to the start");
  assert.equal(addKey(s, 6000, c(0.5, 0.5, 2)), null, "outside");
  const out = addKey(s, 1500, c(0.7, 0.7, 2.5))!;
  assert.deepEqual(out.keys.map((k) => k.t_ms), [1500, 2000]);
  assert.ok(out.manual);
  const follow = addKey(seg({ center: null }), 3000, c(0.6, 0.6, 2))!;
  assert.deepEqual(follow.center, [0.6, 0.6], "a cursor-following segment gets a fixed centre");
});

test("movingASegmentMovesItsKeys", () => {
  const s = moveSegment(seg({ keys: [{ t_ms: 2000, cx: 0.2, cy: 0.2, scale: 2 }] }), 500);
  assert.deepEqual([s.start_ms, s.end_ms, s.keys[0].t_ms], [1500, 5500, 2500]);
  const trimmed = keepKeysInside({ ...seg({ keys: [{ t_ms: 2000, cx: 0.2, cy: 0.2, scale: 2 }, { t_ms: 4000, cx: 0.2, cy: 0.2, scale: 2 }] }), end_ms: 3000 });
  assert.deepEqual(trimmed.keys.map((k) => k.t_ms), [2000], "a key past the new edge is dropped");
});

test("zAtTheEndAddsNothing", () => {
  assert.equal(addSegmentAt([], 9600, 10_000, null), null);
  const later = seg({ start_ms: 3500, end_ms: 6000 });
  const r = addSegmentAt([later], 2000, 10_000, [0.3, 0.7])!;
  assert.equal(r.index, 0);
  assert.deepEqual(r.segments[0], { start_ms: 2000, end_ms: 3500, scale: 1.5, center: [0.3, 0.7], manual: true, keys: [], speed: "normal" });
  const free = addSegmentAt([], 1000, 10_000, null)!;
  assert.deepEqual([free.segments[0].end_ms, free.segments[0].center], [4000, [0.5, 0.5]]);
  assert.equal(addSegmentAt([seg({ start_ms: 2300, end_ms: 6000 })], 2000, 10_000, null), null, "under 0.5 s before the next segment");
});

import { frameAt } from "./zoomMath.ts";

test("the frame shown for editing is the active frame itself", () => {
  const s = seg({ center: [0.3, 0.3], scale: 2, keys: [{ t_ms: 3000, cx: 0.8, cy: 0.7, scale: 3 }] });
  assert.deepEqual(frameAt(s, 2000, c(0.5, 0.5, 1)), c(0.3, 0.3, 2), "main frame, not the eased camera");
  assert.deepEqual(frameAt(s, 3500, c(0.5, 0.5, 1)), c(0.8, 0.7, 3));
  assert.deepEqual(frameAt(seg({ center: null, scale: 2.5 }), 2000, c(0.6, 0.4, 2.2)), c(0.6, 0.4, 2.5), "cursor-following: where the camera is, at the segment's zoom");
});
