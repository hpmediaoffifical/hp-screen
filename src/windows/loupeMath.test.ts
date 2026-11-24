/*! © 2026 HP Media (hpvn.media) — HP Screen. Bảo lưu mọi quyền. HP không có đại lý khác, không hợp tác với đơn vị nào, mọi sao chép đều là vi phạm. */
import assert from "node:assert/strict";
import { test } from "node:test";
import { handlePoint, loupePlace, loupeSource, LOUPE_CELLS } from "./loupeMath.ts";

test("the magnifier is centred on the pixel under the point", () => {
  const s = loupeSource({ x: 100.4, y: 50.6 }, 1920, 1080);
  assert.deepEqual([s.cx, s.cy], [100, 51]);
  assert.equal(s.cx - s.x, (LOUPE_CELLS - 1) / 2);
  assert.equal(s.cy - s.y, (LOUPE_CELLS - 1) / 2);
});

test("a point on the far edge of the screen looks at the last pixel", () => {
  const s = loupeSource({ x: 1920, y: 1080 }, 1920, 1080);
  assert.deepEqual([s.cx, s.cy], [1919, 1079]);
  assert.deepEqual([loupeSource({ x: -5, y: -5 }, 1920, 1080).cx, loupeSource({ x: -5, y: -5 }, 1920, 1080).cy], [0, 0]);
});

test("the magnifier sits away from the inside of the area", () => {
  // Bottom-right corner of an area whose middle is up and to the left: below and to the right.
  const br = loupePlace({ x: 500, y: 400 }, { x: 300, y: 250 }, 138, 160, 1920, 1080);
  assert.ok(br.x > 500 && br.y > 400);
  // Top-left corner: above and to the left.
  const tl = loupePlace({ x: 500, y: 400 }, { x: 700, y: 600 }, 138, 160, 1920, 1080);
  assert.ok(tl.x + 138 < 500 && tl.y + 160 < 400);
});

test("near a screen edge the magnifier flips and always stays on screen", () => {
  const p = loupePlace({ x: 1900, y: 1070 }, { x: 1000, y: 500 }, 138, 160, 1920, 1080);
  assert.ok(p.x + 138 <= 1920 && p.y + 160 <= 1080);
  assert.ok(p.x + 138 < 1900 && p.y + 160 < 1070);
  const q = loupePlace({ x: 2, y: 2 }, { x: 500, y: 500 }, 138, 160, 1920, 1080);
  assert.ok(q.x >= 0 && q.y >= 0);
  // A screen smaller than the magnifier still gets an on-screen corner.
  const tiny = loupePlace({ x: 50, y: 50 }, null, 138, 160, 120, 120);
  assert.ok(tiny.x >= 0 && tiny.y >= 0);
});

test("a resize follows the corner, or the pointer along an edge", () => {
  const box = { x: 100, y: 100, w: 200, h: 100 };
  assert.deepEqual(handlePoint(box, "se", { x: 0, y: 0 }), { x: 300, y: 200 });
  assert.deepEqual(handlePoint(box, "nw", { x: 999, y: 999 }), { x: 100, y: 100 });
  assert.deepEqual(handlePoint(box, "n", { x: 180, y: 999 }), { x: 180, y: 100 });
  assert.deepEqual(handlePoint(box, "e", { x: 0, y: 5000 }), { x: 300, y: 200 });
});
