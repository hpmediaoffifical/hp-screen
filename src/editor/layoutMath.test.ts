import assert from "node:assert/strict";
import { test } from "node:test";
import { pickInView, sourceView } from "./layoutMath.ts";

const whole = { cx: 0.5, cy: 0.5, scale: 1 };
const near = (a: number[], b: number[]) => a.every((v, i) => Math.abs(v - b[i]) < 0.01);

test("fill crops to the frame aspect, like the core", () => {
  assert.ok(near(sourceView(whole, 1920, 1080, 608 / 1080), [656, 0, 608, 1080]));
  assert.ok(near(sourceView(whole, 1920, 1080, null), [0, 0, 1920, 1080]));
  const edge = sourceView({ cx: 0, cy: 0.5, scale: 1 }, 1920, 1080, 608 / 1080);
  assert.equal(edge[0], 0);
  const corner = sourceView({ cx: 0.25, cy: 0.25, scale: 2 }, 1920, 1080, 608 / 1080);
  assert.ok(Math.abs(corner[2] / corner[3] - 608 / 1080) < 1e-3);
});

test("a click in the window picks the source point under it", () => {
  assert.deepEqual(pickInView([0, 0, 1920, 1080], 960, 540, 1920, 1080, 1920, 1080), [0.5, 0.5]);
  assert.deepEqual(pickInView([480, 270, 960, 540], 0, 0, 480, 270, 1920, 1080), [0.25, 0.25]);
});
