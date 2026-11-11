import assert from "node:assert/strict";
import { test } from "node:test";
import { addCut, keptTime, playFrom } from "./timeMath.ts";

const keep = [
  { start_ms: 1000, end_ms: 3000 },
  { start_ms: 5000, end_ms: 9000 },
];

test("kept time skips removed spans", () => {
  assert.equal(keptTime(keep, 500), 1000);
  assert.equal(keptTime(keep, 2000), 2000);
  assert.equal(keptTime(keep, 3000), 5000);
  assert.equal(keptTime(keep, 9000), null);
});

test("play at the end starts again from the beginning", () => {
  assert.equal(playFrom(keep, 8999), 1000);
  assert.equal(playFrom(keep, 9500), 1000);
  assert.equal(playFrom(keep, 4000), 5000);
  assert.equal(playFrom(keep, 2000), 2000);
  assert.equal(playFrom([], 2000), 0);
});

test("overlapping cuts merge into one", () => {
  assert.deepEqual(addCut([{ start_ms: 1000, end_ms: 2000 }], { start_ms: 1500, end_ms: 3000 }), [{ start_ms: 1000, end_ms: 3000 }]);
  assert.deepEqual(addCut([{ start_ms: 5000, end_ms: 6000 }], { start_ms: 1000, end_ms: 2000 }), [
    { start_ms: 1000, end_ms: 2000 },
    { start_ms: 5000, end_ms: 6000 },
  ]);
  assert.deepEqual(
    addCut([{ start_ms: 1000, end_ms: 2000 }, { start_ms: 3000, end_ms: 4000 }], { start_ms: 2000, end_ms: 3000 }),
    [{ start_ms: 1000, end_ms: 4000 }],
  );
  assert.deepEqual(addCut([], { start_ms: 10, end_ms: 10 }), []);
});
