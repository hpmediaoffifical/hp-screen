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
