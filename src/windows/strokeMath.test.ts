import assert from "node:assert/strict";
import { test } from "node:test";
import { alphaAt, segments } from "./strokeMath.ts";
import type { Stroke } from "../liveApi.ts";

const stroke = (tool: Stroke["tool"], points: [number, number][], width = 4): Stroke => ({
  id: 1,
  tool,
  color: [255, 0, 0],
  width,
  points,
  finished_ms: null,
});

test("alpha holds then fades", () => {
  assert.equal(alphaAt(null, 99999, 3000), 1);
  assert.equal(alphaAt(1000, 3999, 3000), 1);
  assert.ok(Math.abs(alphaAt(1000, 4250, 3000) - 0.5) < 0.01);
  assert.equal(alphaAt(1000, 4500, 3000), 0);
  assert.equal(alphaAt(1000, 99999, 0), 1);
});

test("arrow head matches the core", () => {
  const seg = segments(stroke("arrow", [[0, 0], [50, 3], [100, 0]]));
  assert.equal(seg.length, 3);
  assert.deepEqual(seg[0], [[0, 0], [100, 0]]);
  for (const [a, b] of seg.slice(1)) {
    assert.deepEqual(a, [100, 0]);
    assert.ok(Math.abs(Math.hypot(b[0] - a[0], b[1] - a[1]) - 16) < 0.01);
  }
});

test("pen, rect and single points", () => {
  assert.equal(segments(stroke("pen", [[0, 0], [10, 0], [10, 10]])).length, 2);
  const rect = segments(stroke("rect", [[10, 20], [30, 60]]));
  assert.deepEqual(rect.map((s) => s[0]), [[10, 20], [30, 20], [30, 60], [10, 60]]);
  assert.deepEqual(segments(stroke("pen", [[5, 6]])), [[[5, 6], [5, 6]]]);
  assert.deepEqual(segments(stroke("pen", [])), []);
});
