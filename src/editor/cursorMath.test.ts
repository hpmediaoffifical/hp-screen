import assert from "node:assert/strict";
import { test } from "node:test";
import { cursorSampleAt, cursorShapeAt } from "./cursorMath.ts";

const s = (t_ms: number, x: number, alpha = 1, press = 1): [number, number, number, number, number] => [t_ms, x, 0, alpha, press];

test("the drawn cursor is interpolated like the export", () => {
  const samples = [s(0, 0), s(100, 10, 0.5)];
  const c = cursorSampleAt(samples, 50)!;
  assert.equal(c.x, 5);
  assert.equal(c.alpha, 0.75);
  assert.equal(cursorSampleAt(samples, 500)!.x, 10, "held at the end");
  assert.equal(cursorSampleAt([], 5), null);
});

test("the shape is the last one at or before the time, or the first known", () => {
  const fx = { cursor_ids: [[100, 2], [300, 0]] as [number, number][], shapes: [{ id: 1, hx: 0, hy: 0, png: "" }] };
  assert.equal(cursorShapeAt(fx, 50), 1, "before any change: the first shape");
  assert.equal(cursorShapeAt(fx, 200), 2);
  assert.equal(cursorShapeAt(fx, 400), 0, "hidden");
});
