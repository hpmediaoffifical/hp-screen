
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import {
  copyText,
  errorMessage,
  snipCancel,
  snipContext,
  snipFinish,
  snipFrozen,
  snipOcr,
  snipPin,
  snipReady,
  snipScrollStart,
  type OcrResult,
  type SnapWindow,
  type SnipAction,
  type SnipContext,
} from "../api";
import {
  blockArrowOutline,
  commit,
  historyOf,
  moveShape,
  nextStepNumber,
  normalize,
  redo,
  shapeAt,
  shapeBounds,
  stepNumbers,
  undo,
  type Box,
  type History,
  type Point,
  type Shape,
  type TextBg,
  type TextShape,
} from "./snipModel";

/**
 * "Capture and annotate": the frozen screen fills this window. Point at a window and click to
 * take it, or drag to choose an area; adjust it with its handles, draw on it, then copy, save,
 * pin it on the screen or read its text. Everything is kept in the frozen image's physical
 * pixels, so the result is as sharp as the screen.
 */

type Tool = "pen" | "highlight" | "rect" | "ellipse" | "arrow" | "blockArrow" | "line" | "text" | "step" | "mosaic";
type Handle = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";
type Drag =
  | { kind: "select"; start: Point; snap: SnapWindow | null }
  | { kind: "move"; start: Point; origin: Box }
  | { kind: "resize"; handle: Handle; origin: Box }
  | { kind: "draw"; shape: Shape }
  | { kind: "moveShape"; index: number; start: Point; origin: Shape; before: Shape[]; moved: boolean };

const TEXT_BGS: { value: TextBg; label: string }[] = [
  { value: "none", label: "Không nền" },
  { value: "dark", label: "Nền đen" },
  { value: "light", label: "Nền trắng" },
  { value: "orange", label: "Nền vàng cam" },
  { value: "color", label: "Nền theo màu" },
];
const ORANGE = "#ffa41b";
/** Hex colour plus an alpha channel, e.g. "#ff3b30" + 0.8 → "#ff3b30cc". */
const withAlpha = (hex: string, alpha: number) =>
  hex +
  Math.round(alpha * 255)
    .toString(16)
    .padStart(2, "0");

/** Swatches that need dark text on top of them. */
const LIGHT_COLORS = ["#ffcc00", "#34c759", "#ffffff"];

/** Text and box colours for a text annotation. */
function textColors(color: string, bg: TextBg): { fill: string; box: string | null } {
  if (bg === "dark") return { fill: color === "#111111" ? "#ffffff" : color, box: "rgba(0, 0, 0, 0.55)" };
  if (bg === "light") return { fill: color === "#ffffff" ? "#111111" : color, box: "rgba(255, 255, 255, 0.72)" };
  if (bg === "orange") return { fill: "#111111", box: withAlpha(ORANGE, 0.82) };
  if (bg === "color") return { fill: LIGHT_COLORS.includes(color) ? "#111111" : "#ffffff", box: withAlpha(color, 0.8) };
  return { fill: color, box: null };
}

/** Space between text and the edge of its box, as a share of the font size. */
const TEXT_PAD_X = 0.35;
const TEXT_PAD_Y = 0.18;
const LINE_HEIGHT = 1.25;

const measureContext = document.createElement("canvas").getContext("2d")!;

/** Area covered by a text annotation, in image pixels. */
function textBox(s: TextShape): Box {
  measureContext.font = `600 ${s.size}px "Segoe UI", sans-serif`;
  const lines = s.text.split("\n");
  const width = Math.max(...lines.map((l) => measureContext.measureText(l).width));
  return { x: s.at.x, y: s.at.y, w: width, h: lines.length * s.size * LINE_HEIGHT };
}

const COLORS = ["#ff3b30", "#ffcc00", "#34c759", "#0a84ff", "#ffffff", "#111111"];
const COLOR_NAMES = ["Đỏ", "Vàng", "Xanh lá", "Xanh dương", "Trắng", "Đen"];
/** Per size: line width, font size, highlighter width and step radius, in CSS pixels. */
const SIZES = [
  { label: "Mảnh", stroke: 2, font: 16, marker: 12, step: 11 },
  { label: "Vừa", stroke: 4, font: 24, marker: 20, step: 15 },
  { label: "Đậm", stroke: 8, font: 36, marker: 32, step: 21 },
];
const TOOLS: { id: Tool; label: string; icon: ReactNode; key: string }[] = [
  { id: "pen", label: "Bút vẽ tự do", icon: "✎", key: "P" },
  { id: "highlight", label: "Bút tô sáng", icon: <span className="snip-hl-icon" />, key: "H" },
  { id: "rect", label: "Hình chữ nhật", icon: "▭", key: "R" },
  { id: "ellipse", label: "Hình tròn / elip", icon: "◯", key: "O" },
  { id: "arrow", label: "Mũi tên", icon: "↗", key: "A" },
  { id: "blockArrow", label: "Mũi tên khối", icon: "⇨", key: "B" },
  { id: "line", label: "Đường thẳng", icon: "╱", key: "L" },
  { id: "text", label: "Chữ", icon: "T", key: "T" },
  { id: "step", label: "Đánh số bước", icon: "①", key: "N" },
  { id: "mosaic", label: "Làm mờ (che thông tin)", icon: "▦", key: "M" },
];
const HIGHLIGHT_DEFAULT = "#ffcc00";
const HANDLE_CSS = 9; // handle square size, CSS pixels
const MOSAIC_BLOCK_CSS = 10;
/** How close (CSS pixels) a press must be to a line to pick it. */
const PICK_CSS = 5;

function handlePoints(b: Box): [Handle, Point][] {
  const cx = b.x + b.w / 2;
  const cy = b.y + b.h / 2;
  return [
    ["nw", { x: b.x, y: b.y }],
    ["n", { x: cx, y: b.y }],
    ["ne", { x: b.x + b.w, y: b.y }],
    ["e", { x: b.x + b.w, y: cy }],
    ["se", { x: b.x + b.w, y: b.y + b.h }],
    ["s", { x: cx, y: b.y + b.h }],
    ["sw", { x: b.x, y: b.y + b.h }],
    ["w", { x: b.x, y: cy }],
  ];
}

function resizeBox(origin: Box, handle: Handle, p: Point): Box {
  let left = origin.x;
  let top = origin.y;
  let right = origin.x + origin.w;
  let bottom = origin.y + origin.h;
  if (handle.includes("w")) left = p.x;
  if (handle.includes("e")) right = p.x;
  if (handle.includes("n")) top = p.y;
  if (handle.includes("s")) bottom = p.y;
  return normalize({ x: left, y: top }, { x: right, y: bottom });
}

const CURSORS: Record<Handle, string> = {
  nw: "nwse-resize",
  se: "nwse-resize",
  ne: "nesw-resize",
  sw: "nesw-resize",
  n: "ns-resize",
  s: "ns-resize",
  e: "ew-resize",
  w: "ew-resize",
};

/**
 * Draws one annotation; `dx/dy` shift it (used when exporting just the selection). `number`
 * is the label of a step.
 */
function drawShape(ctx: CanvasRenderingContext2D, s: Shape, image: HTMLImageElement, block: number, number: number, dx = 0, dy = 0) {
  ctx.save();
  ctx.translate(-dx, -dy);
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  switch (s.kind) {
    case "pen":
    case "highlight": {
      ctx.strokeStyle = s.color;
      ctx.lineWidth = s.width;
      // A highlighter tints what is under it instead of covering it: multiplying keeps dark
      // text dark and turns the light background around it into the marker colour. The whole
      // stroke is one path, so where it crosses itself it does not get darker.
      if (s.kind === "highlight") ctx.globalCompositeOperation = "multiply";
      ctx.beginPath();
      s.points.forEach((p, i) => (i === 0 ? ctx.moveTo(p.x, p.y) : ctx.lineTo(p.x, p.y)));
      if (s.points.length === 1) ctx.lineTo(s.points[0].x + 0.1, s.points[0].y);
      ctx.stroke();
      break;
    }
    case "rect": {
      const b = normalize(s.from, s.to);
      ctx.strokeStyle = s.color;
      ctx.lineWidth = s.width;
      ctx.strokeRect(b.x, b.y, b.w, b.h);
      break;
    }
    case "ellipse": {
      const b = normalize(s.from, s.to);
      ctx.strokeStyle = s.color;
      ctx.lineWidth = s.width;
      ctx.beginPath();
      ctx.ellipse(b.x + b.w / 2, b.y + b.h / 2, b.w / 2, b.h / 2, 0, 0, Math.PI * 2);
      ctx.stroke();
      break;
    }
    case "line":
    case "arrow": {
      ctx.strokeStyle = s.color;
      ctx.fillStyle = s.color;
      ctx.lineWidth = s.width;
      const angle = Math.atan2(s.to.y - s.from.y, s.to.x - s.from.x);
      const head = Math.max(12, s.width * 4);
      // Stop the shaft at the base of the head so the tip stays sharp.
      const end =
        s.kind === "arrow"
          ? { x: s.to.x - Math.cos(angle) * head * 0.8, y: s.to.y - Math.sin(angle) * head * 0.8 }
          : s.to;
      ctx.beginPath();
      ctx.moveTo(s.from.x, s.from.y);
      ctx.lineTo(end.x, end.y);
      ctx.stroke();
      if (s.kind === "arrow") {
        ctx.beginPath();
        ctx.moveTo(s.to.x, s.to.y);
        ctx.lineTo(s.to.x - head * Math.cos(angle - Math.PI / 7), s.to.y - head * Math.sin(angle - Math.PI / 7));
        ctx.lineTo(s.to.x - head * Math.cos(angle + Math.PI / 7), s.to.y - head * Math.sin(angle + Math.PI / 7));
        ctx.closePath();
        ctx.fill();
      }
      break;
    }
    case "blockArrow": {
      // An outlined "fat" arrow: a shaft and a wide triangular head, drawn as one shape.
      const outline = blockArrowOutline(s.from, s.to, s.width);
      if (outline.length === 0) break;
      ctx.beginPath();
      outline.forEach((p, i) => (i === 0 ? ctx.moveTo(p.x, p.y) : ctx.lineTo(p.x, p.y)));
      ctx.closePath();
      ctx.strokeStyle = s.color;
      ctx.lineWidth = s.width;
      ctx.stroke();
      break;
    }
    case "mosaic": {
      const b = normalize(s.from, s.to);
      if (b.w < 1 || b.h < 1) break;
      // Shrink the area to one pixel per block, then scale it back up without smoothing.
      const small = document.createElement("canvas");
      small.width = Math.max(1, Math.ceil(b.w / block));
      small.height = Math.max(1, Math.ceil(b.h / block));
      small.getContext("2d")!.drawImage(image, b.x, b.y, b.w, b.h, 0, 0, small.width, small.height);
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(small, 0, 0, small.width, small.height, b.x, b.y, small.width * block, small.height * block);
      break;
    }
    case "text": {
      const { fill, box } = textColors(s.color, s.bg);
      if (box) {
        // The box is the text area plus the same padding on both sides, so the text sits
        // in its middle.
        const b = textBox(s);
        const padX = s.size * TEXT_PAD_X;
        const padY = s.size * TEXT_PAD_Y;
        ctx.fillStyle = box;
        ctx.beginPath();
        ctx.roundRect(b.x - padX, b.y - padY, b.w + padX * 2, b.h + padY * 2, s.size * 0.25);
        ctx.fill();
      }
      ctx.font = `600 ${s.size}px "Segoe UI", sans-serif`;
      // Each line is centred vertically in its own line slot, which keeps it centred in the box.
      ctx.textBaseline = "middle";
      ctx.fillStyle = fill;
      const lineHeight = s.size * LINE_HEIGHT;
      s.text.split("\n").forEach((line, i) => ctx.fillText(line, s.at.x, s.at.y + (i + 0.5) * lineHeight));
      break;
    }
    case "step": {
      // A filled disc with a light ring, so it reads on dark and light pictures alike.
      const r = s.size;
      ctx.beginPath();
      ctx.arc(s.at.x, s.at.y, r, 0, Math.PI * 2);
      ctx.fillStyle = s.color;
      ctx.shadowColor = "rgba(0, 0, 0, 0.35)";
      ctx.shadowBlur = r * 0.35;
      ctx.fill();
      ctx.shadowColor = "transparent";
      ctx.lineWidth = Math.max(1.5, r * 0.14);
      ctx.strokeStyle = s.color === "#ffffff" ? "rgba(0, 0, 0, 0.6)" : "rgba(255, 255, 255, 0.92)";
      ctx.stroke();
      const label = String(number);
      ctx.fillStyle = LIGHT_COLORS.includes(s.color) ? "#111111" : "#ffffff";
      ctx.font = `700 ${r * (label.length > 2 ? 0.8 : label.length > 1 ? 1 : 1.2)}px "Segoe UI", sans-serif`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      // Digits sit a little high in Segoe UI's line box; nudge them to the optical centre.
      ctx.fillText(label, s.at.x, s.at.y + r * 0.06);
      break;
    }
  }
  ctx.restore();
}

type OcrState = { busy: true } | { busy: false; result: OcrResult; text: string; copied: boolean };

export default function SnipOverlay() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [image, setImage] = useState<HTMLImageElement | null>(null);
  const [context, setContext] = useState<SnipContext | null>(null);
  const [scale, setScale] = useState(1); // image pixels per CSS pixel
  const [selection, setSelection] = useState<Box | null>(null);
  const [tool, setTool] = useState<Tool | null>(null);
  const [color, setColor] = useState(COLORS[0]);
  const [markerColor, setMarkerColor] = useState(HIGHLIGHT_DEFAULT);
  const [sizeIndex, setSizeIndex] = useState(1);
  const [history, setHistory] = useState<History>(() => historyOf([]));
  const shapes = history.present;
  /** Shape picked with no tool on: it can be dragged or deleted. */
  const [picked, setPicked] = useState<number | null>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [hover, setHover] = useState<Point | null>(null);
  const [textAt, setTextAt] = useState<Point | null>(null);
  const [textValue, setTextValue] = useState("");
  const [textBg, setTextBg] = useState<TextBg>("none");
  /** Shapes as they were before a text was opened for editing (the edit is one undo step). */
  const editBase = useRef<Shape[] | null>(null);
  /** The area whose drawings were cleared when a new area was chosen, and at which undo depth:
   * undoing that step brings the area back with them. */
  const clearedArea = useRef<{ depth: number; area: Box } | null>(null);
  /** Counts text readings, so a reading cancelled with Esc does not reappear when it ends. */
  const ocrRun = useRef(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [ocr, setOcr] = useState<OcrState | null>(null);
  // Measured sizes of the two floating panels (CSS pixels), used to keep them on screen.
  const toolsRef = useRef<HTMLDivElement>(null);
  const actionsRef = useRef<HTMLDivElement>(null);
  const [panels, setPanels] = useState({ tw: 52, th: 420, aw: 300, ah: 48 });
  useLayoutEffect(() => {
    const t = toolsRef.current?.getBoundingClientRect();
    const a = actionsRef.current?.getBoundingClientRect();
    if (!t || !a) return;
    const next = { tw: Math.ceil(t.width), th: Math.ceil(t.height), aw: Math.ceil(a.width), ah: Math.ceil(a.height) };
    if (next.tw !== panels.tw || next.th !== panels.th || next.aw !== panels.aw || next.ah !== panels.ah) setPanels(next);
  });

  // Load the frozen screen, then show the window once it is drawn (no flash of an empty window).
  useEffect(() => {
    // The bytes come through IPC and become a same-origin blob URL. Loading the file through a
    // file URL made the picture "cross-origin", which locks the canvas: every export (copy,
    // save) then failed.
    let url = "";
    snipContext()
      .then(async (ctx) => {
        if (!ctx) return snipCancel();
        setContext(ctx);
        const bytes = await snipFrozen();
        url = URL.createObjectURL(new Blob([bytes], { type: "image/bmp" }));
        const img = new Image();
        img.onload = () => {
          setScale(img.naturalWidth / window.innerWidth || 1);
          setImage(img);
        };
        img.onerror = () => snipCancel();
        img.src = url;
      })
      .catch(() => snipCancel());
    return () => {
      if (url) URL.revokeObjectURL(url);
    };
  }, []);

  const block = MOSAIC_BLOCK_CSS * scale;
  const toImage = (e: { clientX: number; clientY: number }): Point => ({ x: e.clientX * scale, y: e.clientY * scale });
  const inside = (p: Point, b: Box | null) => !!b && p.x >= b.x && p.x <= b.x + b.w && p.y >= b.y && p.y <= b.y + b.h;

  /** The whole screen, offered where no window is (the desktop). */
  const wholeScreen = useMemo<SnapWindow | null>(
    () => (image ? { title: "Cả màn hình", rect: { x: 0, y: 0, w: image.naturalWidth, h: image.naturalHeight } } : null),
    [image],
  );
  /** The top-most window under `p` (windows come top-most first, in image pixels). */
  const windowAt = useCallback(
    (p: Point): SnapWindow | null => context?.windows.find((w) => inside(p, w.rect)) ?? wholeScreen,
    [context, wholeScreen],
  );

  const draft = drag?.kind === "draw" ? drag.shape : null;
  // No area chosen yet (or a press that has not moved): the window under the pointer is offered.
  const choosing = !selection || (drag?.kind === "select" && selection.w < 4 && selection.h < 4);
  const snap = choosing ? (drag?.kind === "select" ? drag.snap : hover ? windowAt(hover) : null) : null;
  const pickedShape = picked !== null && !drag ? (shapes[picked] ?? null) : null;

  /** Edits the shapes as one undo step. */
  const change = useCallback((edit: (shapes: Shape[]) => Shape[]) => setHistory((h) => commit(h, edit(h.present))), []);

  // Repaint everything whenever something visible changes.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !image) return;
    canvas.width = image.naturalWidth;
    canvas.height = image.naturalHeight;
    const ctx = canvas.getContext("2d")!;
    ctx.drawImage(image, 0, 0);
    ctx.fillStyle = "rgba(0, 0, 0, 0.45)";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    if (snap) {
      const { x, y, w, h } = snap.rect;
      ctx.drawImage(image, x, y, w, h, x, y, w, h);
      const line = Math.max(2, 2 * scale);
      ctx.strokeStyle = "#4c8dff";
      ctx.lineWidth = line;
      ctx.strokeRect(x + line / 2, y + line / 2, w - line, h - line);
    }
    if (selection && selection.w > 0 && selection.h > 0) {
      const { x, y, w, h } = selection;
      ctx.drawImage(image, x, y, w, h, x, y, w, h);
      ctx.save();
      ctx.beginPath();
      ctx.rect(x, y, w, h);
      ctx.clip();
      const all = [...shapes, ...(draft ? [draft] : [])];
      const numbers = stepNumbers(all);
      all.forEach((s, i) => drawShape(ctx, s, image, block, numbers[i]));
      if (pickedShape) {
        const b = shapeBounds(pickedShape, textBox);
        const pad = 4 * scale;
        ctx.strokeStyle = "#4c8dff";
        ctx.lineWidth = Math.max(1, 1.5 * scale);
        ctx.setLineDash([5 * scale, 3 * scale]);
        ctx.strokeRect(b.x - pad, b.y - pad, b.w + pad * 2, b.h + pad * 2);
        ctx.setLineDash([]);
      }
      ctx.restore();
      ctx.strokeStyle = "#4c8dff";
      ctx.lineWidth = Math.max(1, scale);
      ctx.setLineDash([6 * scale, 4 * scale]);
      ctx.strokeRect(x + 0.5, y + 0.5, w - 1, h - 1);
      ctx.setLineDash([]);
      const hs = HANDLE_CSS * scale;
      ctx.fillStyle = "#4c8dff";
      handlePoints(selection).forEach(([, p]) => ctx.fillRect(p.x - hs / 2, p.y - hs / 2, hs, hs));
    }
  }, [image, selection, shapes, draft, scale, block, snap, pickedShape]);

  // Tell the core to show the window after the first paint.
  useEffect(() => {
    if (image) requestAnimationFrame(() => snipReady().catch(() => {}));
  }, [image]);

  const hitHandle = (p: Point): Handle | null => {
    if (!selection) return null;
    const r = HANDLE_CSS * scale;
    const found = handlePoints(selection).find(([, h]) => Math.abs(h.x - p.x) <= r && Math.abs(h.y - p.y) <= r);
    return found ? found[0] : null;
  };

  const pendingText = useCallback(
    (): Shape | null =>
      textAt && textValue.trim()
        ? { kind: "text", color, size: SIZES[sizeIndex].font * scale, at: textAt, text: textValue, bg: textBg }
        : null,
    [textAt, textValue, color, sizeIndex, scale, textBg],
  );

  const commitText = useCallback(() => {
    const text = pendingText();
    const base = editBase.current;
    editBase.current = null;
    if (base) {
      // Editing an existing text: the whole edit (even emptying it) is one undo step.
      setHistory((h) => ({ past: [...h.past, base], present: text ? [...h.present, text] : h.present, future: [] }));
    } else if (text) {
      change((s) => [...s, text]);
    }
    setTextAt(null);
    setTextValue("");
  }, [pendingText, change]);

  const cancelText = () => {
    const base = editBase.current;
    editBase.current = null;
    if (base) setHistory((h) => ({ ...h, present: base }));
    setTextAt(null);
    setTextValue("");
  };

  /** What a press at `p` would pick: any shape with no tool on, text with the text tool. */
  const pickAt = (p: Point): number => {
    // Only drawings that can be seen: inside the area.
    if (!selection || !inside(p, selection) || (tool !== null && tool !== "text")) return -1;
    return shapeAt(shapes, p, PICK_CSS * scale, textBox, tool === "text" ? "text" : undefined);
  };

  function onPointerDown(e: ReactPointerEvent<HTMLCanvasElement>) {
    if (e.button !== 0 || !image || busy) return;
    // Without this the browser moves keyboard focus to the page after the press, which
    // immediately closed a text box that this press had just opened.
    e.preventDefault();
    if (textAt) {
      commitText();
      return;
    }
    const p = toImage(e);
    e.currentTarget.setPointerCapture(e.pointerId);
    const handle = hitHandle(p);
    const index = handle ? -1 : pickAt(p);
    setPicked(index >= 0 && tool === null ? index : null);
    if (index >= 0) {
      setDrag({ kind: "moveShape", index, start: p, origin: shapes[index], before: shapes, moved: false });
    } else if (tool && selection && inside(p, selection) && !handle) {
      const size = SIZES[sizeIndex];
      if (tool === "text") {
        setTextAt(p);
        return;
      }
      if (tool === "step") {
        change((s) => [...s, { kind: "step", color, size: size.step * scale, at: p }]);
        return;
      }
      const shape: Shape =
        tool === "pen"
          ? { kind: "pen", color, width: size.stroke * scale, points: [p] }
          : tool === "highlight"
            ? { kind: "highlight", color: markerColor, width: size.marker * scale, points: [p] }
            : tool === "mosaic"
              ? { kind: "mosaic", from: p, to: p }
              : { kind: tool, color, width: size.stroke * scale, from: p, to: p };
      setDrag({ kind: "draw", shape });
    } else if (handle && selection) {
      setDrag({ kind: "resize", handle, origin: selection });
    } else if (!tool && selection && inside(p, selection)) {
      setDrag({ kind: "move", start: p, origin: selection });
    } else if (!tool || !selection) {
      // A new selection starts over: earlier drawings belonged to the old area (undo brings them back).
      if (shapes.length > 0) {
        if (selection) clearedArea.current = { depth: history.past.length + 1, area: selection };
        change(() => []);
      }
      setSelection({ x: p.x, y: p.y, w: 0, h: 0 });
      setDrag({ kind: "select", start: p, snap: windowAt(p) });
    }
  }

  function onPointerMove(e: ReactPointerEvent<HTMLCanvasElement>) {
    const p = toImage(e);
    setHover(p);
    if (!drag || !image) return;
    const clamp = (q: Point): Point => ({
      x: Math.min(Math.max(q.x, 0), image.naturalWidth),
      y: Math.min(Math.max(q.y, 0), image.naturalHeight),
    });
    const q = clamp(p);
    if (drag.kind === "select") setSelection(normalize(drag.start, q));
    if (drag.kind === "resize") setSelection(resizeBox(drag.origin, drag.handle, q));
    if (drag.kind === "move") {
      const o = drag.origin;
      setSelection({
        ...o,
        x: Math.min(Math.max(o.x + p.x - drag.start.x, 0), image.naturalWidth - o.w),
        y: Math.min(Math.max(o.y + p.y - drag.start.y, 0), image.naturalHeight - o.h),
      });
    }
    if (drag.kind === "moveShape") {
      const dx = p.x - drag.start.x;
      const dy = p.y - drag.start.y;
      const moved = drag.moved || Math.hypot(dx, dy) > 3 * scale;
      if (moved) {
        const next = moveShape(drag.origin, dx, dy);
        // Moving is one undo step, recorded when the drag ends.
        setHistory((h) => ({ ...h, present: h.present.map((s, i) => (i === drag.index ? next : s)) }));
      }
      if (moved !== drag.moved) setDrag({ ...drag, moved });
    }
    if (drag.kind === "draw") {
      const s = drag.shape;
      let next: Shape = s;
      if (s.kind === "pen" || s.kind === "highlight") {
        // Shift keeps the stroke straight, which suits underlining a line of text.
        next = { ...s, points: e.shiftKey ? [s.points[0], q] : [...s.points, q] };
      } else if ("to" in s) {
        next = { ...s, to: q };
      }
      setDrag({ kind: "draw", shape: next });
    }
  }

  function onPointerUp() {
    if (!drag || !image) return;
    if (drag.kind === "select" && selection && (selection.w < 4 || selection.h < 4)) {
      // A click without dragging takes the window under the pointer.
      setSelection(drag.snap ? { ...drag.snap.rect } : null);
    }
    if (drag.kind === "draw") {
      // A click with a shape tool draws nothing (a pen click still makes a dot).
      const s = drag.shape;
      const tiny = "to" in s && Math.hypot(s.to.x - s.from.x, s.to.y - s.from.y) < 3 * scale;
      if (!tiny) change((list) => [...list, s]);
    }
    if (drag.kind === "moveShape") {
      if (drag.moved) {
        setHistory((h) => ({ past: [...h.past, drag.before], present: h.present, future: [] }));
      } else if (tool === "text" && drag.origin.kind === "text") {
        // A click (not a drag) on existing text with the text tool edits it.
        const s = drag.origin;
        editBase.current = shapes;
        setHistory((h) => ({ ...h, present: h.present.filter((_, i) => i !== drag.index) }));
        setColor(s.color);
        setTextAt(s.at);
        setTextValue(s.text);
        setTextBg(s.bg);
      }
    }
    setDrag(null);
  }

  /** The selected area with its drawings (or without them, for reading text) as a PNG. */
  const render = useCallback(
    (withDrawings: boolean): { png: string; rect: Box } | null => {
      if (!image || !selection || selection.w < 1 || selection.h < 1) return null;
      const text = pendingText();
      const all = text ? [...shapes, text] : shapes;
      const rect = {
        x: Math.round(selection.x),
        y: Math.round(selection.y),
        w: Math.round(selection.w),
        h: Math.round(selection.h),
      };
      const out = document.createElement("canvas");
      out.width = rect.w;
      out.height = rect.h;
      const ctx = out.getContext("2d")!;
      ctx.drawImage(image, rect.x, rect.y, rect.w, rect.h, 0, 0, rect.w, rect.h);
      if (withDrawings) {
        const numbers = stepNumbers(all);
        all.forEach((s, i) => drawShape(ctx, s, image, block, numbers[i], rect.x, rect.y));
      }
      return { png: out.toDataURL("image/png"), rect };
    },
    [image, selection, shapes, pendingText, block],
  );

  const finish = useCallback(
    async (action: SnipAction) => {
      if (busy) return;
      const out = render(true);
      if (!out) return;
      setBusy(true);
      try {
        await snipFinish(action, out.png);
      } catch (e) {
        setError(errorMessage(e));
        setBusy(false);
      }
    },
    [render, busy],
  );

  /** Keeps the picture on screen in its own small window; the overlay closes. */
  const pin = useCallback(async () => {
    if (busy) return;
    const out = render(true);
    if (!out) return;
    setBusy(true);
    try {
      await snipPin(out.png, out.rect);
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  }, [render, busy]);

  /** Reads the text in the area (from the screen itself, not the drawings). */
  const readText = useCallback(async () => {
    if (busy) return;
    const out = render(false);
    if (!out) return;
    setError("");
    setOcr({ busy: true });
    const run = ++ocrRun.current;
    try {
      const result = await snipOcr(out.png);
      if (run === ocrRun.current) setOcr({ busy: false, result, text: result.text, copied: false });
    } catch (e) {
      if (run !== ocrRun.current) return;
      setOcr(null);
      setError(errorMessage(e));
    }
  }, [render, busy]);

  /** Long pages: the user scrolls the area while it is captured and joined. */
  const scrollCapture = useCallback(async () => {
    if (busy || !selection || !context) return;
    setBusy(true);
    try {
      await snipScrollStart({
        x: Math.round(selection.x),
        y: Math.round(selection.y),
        w: Math.round(selection.w),
        h: Math.round(selection.h),
      });
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  }, [busy, selection, context]);

  const deletePicked = useCallback(() => {
    if (picked === null) return;
    change((s) => s.filter((_, i) => i !== picked));
    setPicked(null);
  }, [picked, change]);
  const doUndo = useCallback(() => {
    setPicked(null);
    const cleared = clearedArea.current;
    if (cleared && cleared.depth === history.past.length) {
      // Undoing "new area cleared the drawings": the old area comes back with them.
      setSelection(cleared.area);
      clearedArea.current = null;
    }
    setHistory(undo);
  }, [history]);
  const doRedo = useCallback(() => {
    setPicked(null);
    setHistory(redo);
  }, []);

  // Keyboard: Esc closes, Ctrl+Z / Ctrl+Y undo and redo, Delete removes the picked drawing,
  // Ctrl+A takes the whole screen, Ctrl+C copies, Ctrl+S saves, Enter does both, F3 pins.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (textAt) return; // the text box handles its own keys
      if (ocr) {
        if (e.key === "Escape") {
          e.preventDefault();
          ocrRun.current++;
          setOcr(null);
        }
        return; // the text panel is open: its own fields take the keys
      }
      const key = e.key.toLowerCase();
      if (e.key === "Escape") {
        e.preventDefault();
        if (picked !== null) setPicked(null);
        else if (tool) setTool(null);
        else snipCancel();
        return;
      }
      if (e.ctrlKey && key === "a" && image) {
        e.preventDefault();
        setSelection({ x: 0, y: 0, w: image.naturalWidth, h: image.naturalHeight });
        return;
      }
      if (!selection) return;
      if (e.ctrlKey && (key === "y" || (key === "z" && e.shiftKey))) {
        e.preventDefault();
        doRedo();
      } else if (e.ctrlKey && key === "z") {
        e.preventDefault();
        doUndo();
      } else if ((e.key === "Delete" || e.key === "Backspace") && picked !== null) {
        e.preventDefault();
        deletePicked();
      } else if (e.ctrlKey && key === "c") {
        e.preventDefault();
        finish("copy");
      } else if (e.ctrlKey && key === "s") {
        e.preventDefault();
        finish("save");
      } else if (e.key === "Enter") {
        e.preventDefault();
        finish("both");
      } else if (e.key === "F3") {
        e.preventDefault();
        pin();
      } else if (!e.ctrlKey && !e.altKey && !e.metaKey) {
        const found = TOOLS.find((t) => t.key === e.key.toUpperCase());
        if (found) {
          setPicked(null);
          setTool((current) => (current === found.id ? null : found.id));
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [tool, selection, textAt, finish, pin, ocr, picked, deletePicked, image, doUndo, doRedo]);

  const nextNumber = useMemo(() => nextStepNumber(shapes), [shapes]);

  if (!image) return <div className="snip" />;

  // Cursor follows what a press would do at the pointer.
  let cursor = "crosshair";
  if (hover && selection && !drag && !choosing) {
    const handle = hitHandle(hover);
    if (handle) cursor = CURSORS[handle];
    else if (pickAt(hover) >= 0) cursor = "move";
    else if (inside(hover, selection)) cursor = tool === "text" ? "text" : tool ? "crosshair" : "move";
  }

  const css = (v: number) => v / scale;
  const sel = selection && selection.w >= 4 && selection.h >= 4 && drag?.kind !== "select" ? selection : null;
  const activeColor = tool === "highlight" ? markerColor : color;
  const setActiveColor = tool === "highlight" ? setMarkerColor : setColor;

  // Place the two panels around the area, flipping sides near the screen edges.
  const GAP = 8;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const clampTo = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), Math.max(lo, hi));
  let toolsLeft = 0;
  let toolsTop = 0;
  let toolsOnLeft = false;
  let actionsLeft = 0;
  let actionsTop = 0;
  if (sel) {
    const left = css(sel.x);
    const right = css(sel.x + sel.w);
    const top = css(sel.y);
    const bottom = css(sel.y + sel.h);
    // Tools: right of the area, else left of it, else inside its right edge.
    if (right + GAP + panels.tw <= vw - GAP) toolsLeft = right + GAP;
    else if (left - GAP - panels.tw >= GAP) {
      toolsLeft = left - GAP - panels.tw;
      toolsOnLeft = true;
    } else toolsLeft = right - GAP - panels.tw;
    toolsTop = clampTo(top, GAP, vh - panels.th - GAP);
    // Actions: under the area, else above it, else inside its bottom edge; right-aligned.
    if (bottom + GAP + panels.ah <= vh - GAP) actionsTop = bottom + GAP;
    else if (top - GAP - panels.ah >= GAP) actionsTop = top - GAP - panels.ah;
    else actionsTop = bottom - GAP - panels.ah;
    actionsLeft = clampTo(right - panels.aw, GAP, vw - panels.aw - GAP);
  }

  const toolLabel = (t: (typeof TOOLS)[number]) =>
    t.id === "step" ? `${t.label} (${t.key}) · số tiếp theo: ${nextNumber}` : `${t.label} (${t.key})`;

  return (
    <div className="snip">
      <canvas
        ref={canvasRef}
        className="snip-canvas"
        style={{ cursor }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerLeave={() => setHover(null)}
        // Double-click on the area (not on a drawing) finishes, like the ✓ button.
        onDoubleClick={(e) => !tool && pickAt(toImage(e)) < 0 && finish("both")}
      />

      {!sel && (
        <p className="snip-hint">
          Bấm vào một cửa sổ để chụp đúng khung · Kéo chuột để tự chọn vùng · Ctrl+A: cả màn hình · Esc để hủy
        </p>
      )}

      {snap && (
        <span className="snip-size snip-snap-tag" style={{ left: css(snap.rect.x) + 4, top: css(snap.rect.y) + 4 }}>
          {snap.title} · {snap.rect.w} × {snap.rect.h}
        </span>
      )}

      {selection && selection.w > 0 && !choosing && (
        <span className="snip-size" style={{ left: css(selection.x), top: Math.max(4, css(selection.y) - 26) }}>
          {Math.round(selection.w)} × {Math.round(selection.h)}
        </span>
      )}

      {textAt && (
        <textarea
          className="snip-text"
          autoFocus
          ref={(el) => {
            // Focus after the press that opened the box has finished.
            if (el && document.activeElement !== el) requestAnimationFrame(() => el.focus());
          }}
          onPointerDown={(e) => e.stopPropagation()}
          rows={1}
          aria-label="Nội dung chữ"
          placeholder="Nhập chữ…"
          value={textValue}
          style={{
            left: css(textAt.x),
            top: css(textAt.y),
            fontSize: SIZES[sizeIndex].font,
            color: textColors(color, textBg).fill,
            background: textColors(color, textBg).box ?? "rgba(0, 0, 0, 0.25)",
          }}
          onChange={(e) => setTextValue(e.target.value)}
          onBlur={commitText}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              commitText();
            } else if (e.key === "Escape") {
              e.preventDefault();
              cancelText();
            }
          }}
        />
      )}

      {sel && (
        <>
          {/* Drawing tools: a column beside the area, plus their options when a tool is on. */}
          <div
            ref={toolsRef}
            className={"snip-panel snip-tools" + (toolsOnLeft ? " snip-flip" : "")}
            style={{ left: toolsLeft, top: toolsTop }}
            onPointerDown={(e) => e.stopPropagation()}
          >
            <div className="snip-col" role="toolbar" aria-orientation="vertical" aria-label="Công cụ vẽ">
              {TOOLS.map((t) => (
                <button
                  key={t.id}
                  className={"snip-btn" + (tool === t.id ? " on" : "")}
                  title={toolLabel(t)}
                  aria-label={t.label}
                  aria-pressed={tool === t.id}
                  onClick={() => {
                    setPicked(null);
                    setTool(tool === t.id ? null : t.id);
                  }}
                >
                  {t.icon}
                </button>
              ))}
              <span className="snip-hsep" />
              <button className="snip-btn" title="Hoàn tác (Ctrl+Z)" aria-label="Hoàn tác" disabled={history.past.length === 0} onClick={doUndo}>
                ↶
              </button>
              <button
                className="snip-btn"
                title="Làm lại (Ctrl+Y)"
                aria-label="Làm lại"
                disabled={history.future.length === 0}
                onClick={doRedo}
              >
                ↷
              </button>
              <button
                className="snip-btn"
                title={picked === null ? "Xóa nét đang chọn (Delete) · tắt công cụ rồi bấm vào một nét để chọn" : "Xóa nét đang chọn (Delete)"}
                aria-label="Xóa nét đang chọn"
                disabled={picked === null}
                onClick={deletePicked}
              >
                ⌫
              </button>
            </div>
            {tool && tool !== "mosaic" && (
              <div className="snip-col snip-options">
                <div className="snip-col" role="radiogroup" aria-label="Màu">
                  {COLORS.map((c, i) => (
                    <button
                      key={c}
                      role="radio"
                      className={"snip-swatch" + (activeColor === c ? " on" : "")}
                      style={{ background: c }}
                      title={COLOR_NAMES[i]}
                      aria-label={`Màu ${COLOR_NAMES[i]}`}
                      aria-checked={activeColor === c}
                      onClick={() => setActiveColor(c)}
                    />
                  ))}
                </div>
                <span className="snip-hsep" />
                <div className="snip-col" role="radiogroup" aria-label={tool === "text" ? "Cỡ chữ" : "Cỡ nét"}>
                  {SIZES.map((s, i) => {
                    const name = `${tool === "text" ? "Chữ" : tool === "step" ? "Số" : "Nét"} ${s.label.toLowerCase()}`;
                    return (
                      <button
                        key={s.label}
                        role="radio"
                        className={"snip-btn snip-size-btn" + (sizeIndex === i ? " on" : "")}
                        title={name}
                        aria-label={name}
                        aria-checked={sizeIndex === i}
                        onClick={() => setSizeIndex(i)}
                      >
                        <span className="snip-dot" style={{ width: 4 + s.stroke * 1.5, height: 4 + s.stroke * 1.5, background: activeColor }} />
                      </button>
                    );
                  })}
                </div>
                {tool === "text" && (
                  <>
                    <span className="snip-hsep" />
                    <div className="snip-col" role="radiogroup" aria-label="Nền chữ">
                      {TEXT_BGS.map((b) => (
                        <button
                          key={b.value}
                          role="radio"
                          aria-checked={textBg === b.value}
                          className={"snip-btn snip-bg-btn" + (textBg === b.value ? " on" : "")}
                          title={b.label}
                          aria-label={b.label}
                          onClick={() => setTextBg(b.value)}
                        >
                          <span
                            className={"snip-bg-sample snip-bg-" + b.value}
                            style={b.value === "color" ? { background: color, color: textColors(color, "color").fill } : undefined}
                          >
                            Aa
                          </span>
                        </button>
                      ))}
                    </div>
                  </>
                )}
              </div>
            )}
          </div>

          {/* Finishing actions: under the area, or above it near the bottom edge. */}
          <div
            ref={actionsRef}
            className="snip-panel snip-actions"
            style={{ left: actionsLeft, top: actionsTop }}
            onPointerDown={(e) => e.stopPropagation()}
          >
            <div className="snip-row" role="toolbar" aria-label="Lưu và sao chép">
              <button className="snip-btn" title="Hủy (Esc)" aria-label="Hủy" onClick={() => snipCancel()}>
                ✕
              </button>
              <button
                className="snip-btn snip-wide"
                title="Chụp trang dài: cuộn trang trong vùng này, HP Screen ghép lại thành một ảnh"
                disabled={busy}
                onClick={scrollCapture}
              >
                ⇕ Chụp cuộn
              </button>
              <button className="snip-btn snip-wide" title="Nhận chữ trong vùng chọn (OCR)" disabled={busy || ocr?.busy} onClick={readText}>
                {ocr?.busy ? "Đang đọc…" : "Nhận chữ"}
              </button>
              <button className="snip-btn snip-wide" title="Ghim ảnh nổi trên màn hình (F3)" disabled={busy} onClick={pin}>
                📌 Ghim
              </button>
              <span className="snip-sep" />
              <button className="snip-btn snip-wide" title="Lưu vào máy (Ctrl+S)" disabled={busy} onClick={() => finish("save")}>
                Lưu
              </button>
              <button className="snip-btn snip-wide" title="Sao chép để dán (Ctrl+C)" disabled={busy} onClick={() => finish("copy")}>
                Sao chép
              </button>
              <button className="snip-btn snip-wide snip-done" title="Lưu và sao chép (Enter)" disabled={busy} onClick={() => finish("both")}>
                ✓ Xong
              </button>
            </div>
            {error && <p className="snip-error" role="alert">{error}</p>}
          </div>
        </>
      )}

      {ocr && !ocr.busy && (
        <div className="snip-ocr" role="dialog" aria-label="Chữ nhận được" onPointerDown={(e) => e.stopPropagation()}>
          <div className="snip-ocr-head">
            <strong>Chữ nhận được</strong>
            <span className="snip-label">Đọc bằng {ocr.result.language}</span>
          </div>
          {ocr.result.note && <p className="snip-ocr-note">{ocr.result.note}</p>}
          <textarea
            className="snip-ocr-text"
            autoFocus
            spellCheck={false}
            aria-label="Chữ nhận được, có thể sửa"
            placeholder="Không tìm thấy chữ nào trong vùng này."
            value={ocr.text}
            onChange={(e) => setOcr({ ...ocr, text: e.target.value, copied: false })}
          />
          <div className="snip-row snip-ocr-actions">
            <button className="snip-btn snip-wide" onClick={() => setOcr(null)}>
              Đóng
            </button>
            <button
              className="snip-btn snip-wide snip-done"
              disabled={!ocr.text.trim()}
              onClick={() =>
                copyText(ocr.text)
                  .then(() => setOcr({ ...ocr, copied: true }))
                  .catch((e) => setError(errorMessage(e)))
              }
            >
              {ocr.copied ? "✓ Đã sao chép" : "Sao chép chữ"}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
