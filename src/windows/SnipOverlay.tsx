import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { errorMessage, snipCancel, snipContext, snipFinish, snipReady, type SnipAction } from "../api";

/**
 * "Capture and annotate": the frozen screen fills this window. Drag to choose an area, adjust
 * it with its handles, draw on it, then copy and/or save. Everything is kept in the frozen
 * image's physical pixels, so the result is as sharp as the screen.
 */

type Tool = "pen" | "rect" | "ellipse" | "arrow" | "line" | "text" | "mosaic";
interface Point {
  x: number;
  y: number;
}
interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}
type Shape =
  | { kind: "pen"; color: string; width: number; points: Point[] }
  | { kind: "rect" | "ellipse" | "arrow" | "line"; color: string; width: number; from: Point; to: Point }
  | { kind: "mosaic"; from: Point; to: Point }
  | { kind: "text"; color: string; size: number; at: Point; text: string };
type Handle = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";
type Drag =
  | { kind: "select"; start: Point }
  | { kind: "move"; start: Point; origin: Box }
  | { kind: "resize"; handle: Handle; origin: Box }
  | { kind: "draw"; shape: Shape }
  | { kind: "moveText"; index: number; start: Point; origin: Point; moved: boolean };

type TextShape = Extract<Shape, { kind: "text" }>;

const measureContext = document.createElement("canvas").getContext("2d")!;

/** Area covered by a text annotation, in image pixels. */
function textBox(s: TextShape): Box {
  measureContext.font = `600 ${s.size}px "Segoe UI", sans-serif`;
  const lines = s.text.split("\n");
  const width = Math.max(...lines.map((l) => measureContext.measureText(l).width));
  return { x: s.at.x, y: s.at.y, w: width, h: lines.length * s.size * 1.25 };
}

const COLORS = ["#ff3b30", "#ffcc00", "#34c759", "#0a84ff", "#ffffff", "#111111"];
const COLOR_NAMES = ["Đỏ", "Vàng", "Xanh lá", "Xanh dương", "Trắng", "Đen"];
const SIZES = [
  { label: "Mảnh", stroke: 2, font: 16 },
  { label: "Vừa", stroke: 4, font: 24 },
  { label: "Đậm", stroke: 8, font: 36 },
];
const TOOLS: { id: Tool; label: string; icon: string; key: string }[] = [
  { id: "pen", label: "Bút vẽ tự do", icon: "✎", key: "P" },
  { id: "rect", label: "Hình chữ nhật", icon: "▭", key: "R" },
  { id: "ellipse", label: "Hình tròn / elip", icon: "◯", key: "O" },
  { id: "arrow", label: "Mũi tên", icon: "↗", key: "A" },
  { id: "line", label: "Đường thẳng", icon: "╱", key: "L" },
  { id: "text", label: "Chữ", icon: "T", key: "T" },
  { id: "mosaic", label: "Làm mờ (che thông tin)", icon: "▦", key: "M" },
];
const HANDLE_CSS = 9; // handle square size, CSS pixels
const MOSAIC_BLOCK_CSS = 10;

const normalize = (a: Point, b: Point): Box => ({
  x: Math.min(a.x, b.x),
  y: Math.min(a.y, b.y),
  w: Math.abs(b.x - a.x),
  h: Math.abs(b.y - a.y),
});

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

/** Draws one annotation; `dx/dy` shift it (used when exporting just the selection). */
function drawShape(ctx: CanvasRenderingContext2D, s: Shape, image: HTMLImageElement, block: number, dx = 0, dy = 0) {
  ctx.save();
  ctx.translate(-dx, -dy);
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  switch (s.kind) {
    case "pen": {
      ctx.strokeStyle = s.color;
      ctx.lineWidth = s.width;
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
      ctx.fillStyle = s.color;
      ctx.font = `600 ${s.size}px "Segoe UI", sans-serif`;
      ctx.textBaseline = "top";
      s.text.split("\n").forEach((line, i) => ctx.fillText(line, s.at.x, s.at.y + i * s.size * 1.25));
      break;
    }
  }
  ctx.restore();
}

export default function SnipOverlay() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [image, setImage] = useState<HTMLImageElement | null>(null);
  const [scale, setScale] = useState(1); // image pixels per CSS pixel
  const [selection, setSelection] = useState<Box | null>(null);
  const [tool, setTool] = useState<Tool | null>(null);
  const [color, setColor] = useState(COLORS[0]);
  const [sizeIndex, setSizeIndex] = useState(1);
  const [shapes, setShapes] = useState<Shape[]>([]);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [hover, setHover] = useState<Point | null>(null);
  const [textAt, setTextAt] = useState<Point | null>(null);
  const [textValue, setTextValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  // Load the frozen screen, then show the window once it is drawn (no flash of an empty window).
  useEffect(() => {
    snipContext()
      .then((ctx) => {
        if (!ctx) return snipCancel();
        const img = new Image();
        img.onload = () => {
          setScale(img.naturalWidth / window.innerWidth || 1);
          setImage(img);
        };
        img.onerror = () => snipCancel();
        img.src = convertFileSrc(ctx.frozen) + `?t=${Date.now()}`;
      })
      .catch(() => snipCancel());
  }, []);

  const block = MOSAIC_BLOCK_CSS * scale;
  const toImage = (e: { clientX: number; clientY: number }): Point => ({ x: e.clientX * scale, y: e.clientY * scale });

  const draft = drag?.kind === "draw" ? drag.shape : null;

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
    if (selection && selection.w > 0 && selection.h > 0) {
      const { x, y, w, h } = selection;
      ctx.drawImage(image, x, y, w, h, x, y, w, h);
      ctx.save();
      ctx.beginPath();
      ctx.rect(x, y, w, h);
      ctx.clip();
      [...shapes, ...(draft ? [draft] : [])].forEach((s) => drawShape(ctx, s, image, block));
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
  }, [image, selection, shapes, draft, scale, block]);

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
  const inside = (p: Point, b: Box | null) => !!b && p.x >= b.x && p.x <= b.x + b.w && p.y >= b.y && p.y <= b.y + b.h;

  const commitText = useCallback(() => {
    if (textAt && textValue.trim()) {
      setShapes((s) => [...s, { kind: "text", color, size: SIZES[sizeIndex].font * scale, at: textAt, text: textValue }]);
    }
    setTextAt(null);
    setTextValue("");
  }, [textAt, textValue, color, sizeIndex, scale]);

  /** Index of the top-most text annotation under `p`, if any. */
  const hitText = (p: Point): number => {
    const pad = 4 * scale;
    for (let i = shapes.length - 1; i >= 0; i--) {
      const s = shapes[i];
      if (s.kind !== "text") continue;
      const b = textBox(s);
      if (p.x >= b.x - pad && p.x <= b.x + b.w + pad && p.y >= b.y - pad && p.y <= b.y + b.h + pad) return i;
    }
    return -1;
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
    const textIndex = (tool === null || tool === "text") && selection && !handle ? hitText(p) : -1;
    if (textIndex >= 0) {
      const s = shapes[textIndex] as TextShape;
      setDrag({ kind: "moveText", index: textIndex, start: p, origin: s.at, moved: false });
    } else if (tool && selection && inside(p, selection) && !handle) {
      const width = SIZES[sizeIndex].stroke * scale;
      if (tool === "text") {
        setTextAt(p);
        return;
      }
      const shape: Shape =
        tool === "pen"
          ? { kind: "pen", color, width, points: [p] }
          : tool === "mosaic"
            ? { kind: "mosaic", from: p, to: p }
            : { kind: tool, color, width, from: p, to: p };
      setDrag({ kind: "draw", shape });
    } else if (handle && selection) {
      setDrag({ kind: "resize", handle, origin: selection });
    } else if (!tool && selection && inside(p, selection)) {
      setDrag({ kind: "move", start: p, origin: selection });
    } else if (!tool || !selection) {
      // A new selection starts over: earlier drawings belonged to the old area.
      setShapes([]);
      setSelection({ x: p.x, y: p.y, w: 0, h: 0 });
      setDrag({ kind: "select", start: p });
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
    if (drag.kind === "moveText") {
      const dx = p.x - drag.start.x;
      const dy = p.y - drag.start.y;
      const moved = drag.moved || Math.hypot(dx, dy) > 3 * scale;
      if (moved) {
        const at = { x: drag.origin.x + dx, y: drag.origin.y + dy };
        setShapes((list) => list.map((s, i) => (i === drag.index && s.kind === "text" ? { ...s, at } : s)));
      }
      if (moved !== drag.moved) setDrag({ ...drag, moved });
    }
    if (drag.kind === "draw") {
      const s = drag.shape;
      setDrag({ kind: "draw", shape: s.kind === "pen" ? { ...s, points: [...s.points, q] } : s.kind === "text" ? s : { ...s, to: q } });
    }
  }

  function onPointerUp() {
    if (!drag || !image) return;
    if (drag.kind === "select" && selection && (selection.w < 4 || selection.h < 4)) {
      // A click without dragging takes the whole screen.
      setSelection({ x: 0, y: 0, w: image.naturalWidth, h: image.naturalHeight });
    }
    if (drag.kind === "draw") setShapes((s) => [...s, drag.shape]);
    if (drag.kind === "moveText" && !drag.moved && tool === "text") {
      // A click (not a drag) on existing text with the text tool edits it.
      const s = shapes[drag.index] as TextShape;
      setShapes((list) => list.filter((_, i) => i !== drag.index));
      setColor(s.color);
      setTextAt(s.at);
      setTextValue(s.text);
    }
    setDrag(null);
  }

  const finish = useCallback(
    async (action: SnipAction) => {
      if (!image || !selection || selection.w < 1 || selection.h < 1 || busy) return;
      const pending: Shape[] =
        textAt && textValue.trim()
          ? [...shapes, { kind: "text", color, size: SIZES[sizeIndex].font * scale, at: textAt, text: textValue }]
          : shapes;
      const sel = {
        x: Math.round(selection.x),
        y: Math.round(selection.y),
        w: Math.round(selection.w),
        h: Math.round(selection.h),
      };
      const out = document.createElement("canvas");
      out.width = sel.w;
      out.height = sel.h;
      const ctx = out.getContext("2d")!;
      ctx.drawImage(image, sel.x, sel.y, sel.w, sel.h, 0, 0, sel.w, sel.h);
      pending.forEach((s) => drawShape(ctx, s, image, block, sel.x, sel.y));
      setBusy(true);
      try {
        await snipFinish(action, out.toDataURL("image/png"));
      } catch (e) {
        setError(errorMessage(e));
        setBusy(false);
      }
    },
    [image, selection, shapes, textAt, textValue, color, sizeIndex, scale, block, busy],
  );

  // Keyboard: Esc closes, Ctrl+Z undoes, Ctrl+C copies, Ctrl+S saves, Enter does both.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (textAt) return; // the text box handles its own keys
      if (e.key === "Escape") {
        e.preventDefault();
        if (tool) setTool(null);
        else snipCancel();
        return;
      }
      if (!selection) return;
      if (e.ctrlKey && e.key.toLowerCase() === "z") {
        e.preventDefault();
        setShapes((s) => s.slice(0, -1));
      } else if (e.ctrlKey && e.key.toLowerCase() === "c") {
        e.preventDefault();
        finish("copy");
      } else if (e.ctrlKey && e.key.toLowerCase() === "s") {
        e.preventDefault();
        finish("save");
      } else if (e.key === "Enter") {
        e.preventDefault();
        finish("both");
      } else if (!e.ctrlKey && !e.altKey && !e.metaKey) {
        const picked = TOOLS.find((t) => t.key === e.key.toUpperCase());
        if (picked) setTool((current) => (current === picked.id ? null : picked.id));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [tool, selection, textAt, finish]);

  if (!image) return <div className="snip" />;

  // Cursor follows what a press would do at the pointer.
  let cursor = "crosshair";
  if (hover && selection && !drag) {
    const handle = hitHandle(hover);
    if (handle) cursor = CURSORS[handle];
    else if ((tool === null || tool === "text") && hitText(hover) >= 0) cursor = "move";
    else if (inside(hover, selection)) cursor = tool === "text" ? "text" : tool ? "crosshair" : "move";
  }

  const css = (v: number) => v / scale;
  const sel = selection && selection.w >= 4 && selection.h >= 4 && drag?.kind !== "select" ? selection : null;
  const toolbarBelow = sel && css(sel.y + sel.h) + 110 < window.innerHeight;
  const toolbarAbove = sel && !toolbarBelow && css(sel.y) > 110;
  const toolbarStyle = sel
    ? {
        right: Math.max(8, window.innerWidth - css(sel.x + sel.w)),
        top: toolbarBelow ? css(sel.y + sel.h) + 10 : toolbarAbove ? undefined : undefined,
        bottom: toolbarAbove ? window.innerHeight - css(sel.y) + 10 : toolbarBelow ? undefined : 16,
      }
    : undefined;

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
        onDoubleClick={() => !tool && finish("both")}
      />

      {!sel && <p className="snip-hint">Kéo chuột để chọn vùng chụp · Bấm một lần để chụp cả màn hình · Esc để hủy</p>}

      {selection && selection.w > 0 && (
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
          style={{ left: css(textAt.x), top: css(textAt.y), color, fontSize: SIZES[sizeIndex].font }}
          onChange={(e) => setTextValue(e.target.value)}
          onBlur={commitText}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              commitText();
            } else if (e.key === "Escape") {
              e.preventDefault();
              setTextAt(null);
              setTextValue("");
            }
          }}
        />
      )}

      {sel && (
        <div className="snip-toolbar" style={toolbarStyle} onPointerDown={(e) => e.stopPropagation()}>
          <div className="snip-row" role="toolbar" aria-label="Công cụ vẽ">
            {TOOLS.map((t) => (
              <button
                key={t.id}
                className={"snip-btn" + (tool === t.id ? " on" : "")}
                title={`${t.label} (${t.key})`}
                aria-label={t.label}
                aria-pressed={tool === t.id}
                onClick={() => setTool(tool === t.id ? null : t.id)}
              >
                {t.icon}
              </button>
            ))}
            <span className="snip-sep" />
            <button
              className="snip-btn"
              title="Hoàn tác (Ctrl+Z)"
              aria-label="Hoàn tác"
              disabled={shapes.length === 0}
              onClick={() => setShapes((s) => s.slice(0, -1))}
            >
              ↶
            </button>
            <span className="snip-sep" />
            <button className="snip-btn" title="Hủy (Esc)" aria-label="Hủy" onClick={() => snipCancel()}>
              ✕
            </button>
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
          {tool && tool !== "mosaic" && (
            <div className="snip-row" aria-label="Màu và cỡ nét">
              {COLORS.map((c, i) => (
                <button
                  key={c}
                  className={"snip-swatch" + (color === c ? " on" : "")}
                  style={{ background: c }}
                  title={COLOR_NAMES[i]}
                  aria-label={`Màu ${COLOR_NAMES[i]}`}
                  aria-pressed={color === c}
                  onClick={() => setColor(c)}
                />
              ))}
              <span className="snip-sep" />
              {SIZES.map((s, i) => (
                <button
                  key={s.label}
                  className={"snip-btn snip-size-btn" + (sizeIndex === i ? " on" : "")}
                  title={tool === "text" ? `Chữ ${s.label.toLowerCase()}` : `Nét ${s.label.toLowerCase()}`}
                  aria-label={tool === "text" ? `Chữ ${s.label.toLowerCase()}` : `Nét ${s.label.toLowerCase()}`}
                  aria-pressed={sizeIndex === i}
                  onClick={() => setSizeIndex(i)}
                >
                  <span className="snip-dot" style={{ width: 4 + s.stroke * 1.5, height: 4 + s.stroke * 1.5, background: color }} />
                </button>
              ))}
            </div>
          )}
          {error && <p className="snip-error" role="alert">{error}</p>}
        </div>
      )}
    </div>
  );
}
