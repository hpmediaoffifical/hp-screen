// Pure logic of the capture-and-annotate overlay. Run with: node --test tests/
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  commit,
  hitShape,
  historyOf,
  moveShape,
  nextStepNumber,
  redo,
  shapeAt,
  shapeBounds,
  stepNumbers,
  undo,
  type Box,
  type Shape,
} from "../src/windows/snipModel.ts";

const step = (x: number, y: number): Shape => ({ kind: "step", color: "#ff3b30", size: 16, at: { x, y } });
const rect: Shape = { kind: "rect", color: "#fff", width: 4, from: { x: 100, y: 100 }, to: { x: 300, y: 200 } };
const noText = (): Box => ({ x: 0, y: 0, w: 0, h: 0 });

test("steps are numbered in the order they were placed, other shapes skipped", () => {
  const shapes = [step(0, 0), rect, step(10, 10), step(20, 20)];
  assert.deepEqual(stepNumbers(shapes), [1, 0, 2, 3]);
  assert.equal(nextStepNumber(shapes), 4);
  assert.equal(nextStepNumber([rect]), 1);
});

test("undoing a step brings the next number back, redo moves it on again", () => {
  let h = historyOf([]);
  h = commit(h, [step(0, 0)]);
  h = commit(h, [...h.present, step(10, 0)]);
  assert.equal(nextStepNumber(h.present), 3);
  h = undo(h);
  assert.equal(nextStepNumber(h.present), 2);
  h = redo(h);
  assert.equal(nextStepNumber(h.present), 3);
});

test("deleting a step renumbers the ones after it", () => {
  const shapes = [step(0, 0), step(10, 0), step(20, 0)];
  assert.deepEqual(stepNumbers(shapes.filter((_, i) => i !== 1)), [1, 2]);
});

test("history: undo and redo walk back and forth, a new change drops the redo list", () => {
  let h = historyOf([]);
  h = commit(h, [rect]);
  h = commit(h, [rect, step(0, 0)]);
  h = undo(h);
  assert.deepEqual(h.present, [rect]);
  h = undo(h);
  assert.deepEqual(h.present, []);
  assert.equal(undo(h), h, "nothing left to undo");
  h = redo(h);
  assert.deepEqual(h.present, [rect]);
  h = commit(h, [rect, rect]);
  assert.equal(redo(h), h, "redo list cleared by the new change");
  assert.equal(h.past.length, 2);
});

test("a rectangle is picked on its outline, not in its empty middle", () => {
  assert.ok(hitShape(rect, { x: 102, y: 150 }, 4, noText));
  assert.ok(hitShape(rect, { x: 200, y: 205 }, 4, noText));
  assert.ok(!hitShape(rect, { x: 200, y: 150 }, 4, noText));
  assert.ok(!hitShape(rect, { x: 320, y: 150 }, 4, noText));
});

test("lines, arrows and pen strokes are picked near their path", () => {
  const arrow: Shape = { kind: "arrow", color: "#fff", width: 4, from: { x: 0, y: 0 }, to: { x: 100, y: 100 } };
  assert.ok(hitShape(arrow, { x: 52, y: 48 }, 4, noText));
  assert.ok(!hitShape(arrow, { x: 80, y: 20 }, 4, noText));
  const pen: Shape = { kind: "pen", color: "#fff", width: 4, points: [{ x: 0, y: 0 }, { x: 50, y: 0 }, { x: 50, y: 50 }] };
  assert.ok(hitShape(pen, { x: 51, y: 30 }, 4, noText));
  assert.ok(!hitShape(pen, { x: 20, y: 30 }, 4, noText));
  const marker: Shape = { kind: "highlight", color: "#ffcc00", width: 30, points: [{ x: 0, y: 100 }, { x: 200, y: 100 }] };
  assert.ok(hitShape(marker, { x: 100, y: 112 }, 2, noText), "inside the wide stroke");
});

test("ellipses are picked on their outline; block arrows and mosaics anywhere inside", () => {
  const ellipse: Shape = { kind: "ellipse", color: "#fff", width: 4, from: { x: 0, y: 0 }, to: { x: 200, y: 100 } };
  assert.ok(hitShape(ellipse, { x: 200, y: 50 }, 4, noText));
  assert.ok(hitShape(ellipse, { x: 100, y: 1 }, 4, noText));
  assert.ok(!hitShape(ellipse, { x: 100, y: 50 }, 4, noText));
  const block: Shape = { kind: "blockArrow", color: "#fff", width: 4, from: { x: 0, y: 100 }, to: { x: 300, y: 100 } };
  assert.ok(hitShape(block, { x: 100, y: 100 }, 4, noText));
  assert.ok(!hitShape(block, { x: 100, y: 200 }, 4, noText));
  const mosaic: Shape = { kind: "mosaic", from: { x: 10, y: 10 }, to: { x: 60, y: 60 } };
  assert.ok(hitShape(mosaic, { x: 30, y: 30 }, 0, noText));
});

test("text is picked inside its measured box; steps inside their circle", () => {
  const text: Shape = { kind: "text", color: "#fff", size: 20, at: { x: 50, y: 50 }, text: "Xin chào", bg: "none" };
  const measure = () => ({ x: 50, y: 50, w: 80, h: 25 });
  assert.ok(hitShape(text, { x: 90, y: 60 }, 4, measure));
  assert.ok(!hitShape(text, { x: 200, y: 60 }, 4, measure));
  assert.ok(hitShape(step(100, 100), { x: 110, y: 108 }, 2, noText));
  assert.ok(!hitShape(step(100, 100), { x: 125, y: 100 }, 2, noText));
});

test("the top-most shape under the pointer wins", () => {
  const shapes = [step(100, 100), step(105, 100), rect];
  assert.equal(shapeAt(shapes, { x: 104, y: 90 }, 2, noText), 1);
  assert.equal(shapeAt(shapes, { x: 500, y: 500 }, 2, noText), -1);
});

test("moving a shape shifts every point it has", () => {
  const pen: Shape = { kind: "pen", color: "#fff", width: 2, points: [{ x: 1, y: 2 }, { x: 3, y: 4 }] };
  assert.deepEqual(moveShape(pen, 10, -1), { ...pen, points: [{ x: 11, y: 1 }, { x: 13, y: 3 }] });
  assert.deepEqual(moveShape(rect, 5, 5), { ...rect, from: { x: 105, y: 105 }, to: { x: 305, y: 205 } });
  assert.deepEqual(moveShape(step(0, 0), 3, 4), step(3, 4));
});

test("bounds cover the drawn shape including its stroke", () => {
  assert.deepEqual(shapeBounds(rect, noText), { x: 98, y: 98, w: 204, h: 104 });
  assert.deepEqual(shapeBounds(step(50, 50), noText), { x: 34, y: 34, w: 32, h: 32 });
});
