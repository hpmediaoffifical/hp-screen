import { useEffect, useRef, useState } from "react";
import { errorMessage } from "../api";
import { drawClear, drawContext, drawSet, drawStroke, onDrawMode, type Stroke, type Tool } from "../liveApi";
import { alphaAt, COLORS, segments, WIDTHS } from "./strokeMath";

interface Shown {
  stroke: Stroke;
  /** performance.now() when finished; null while drawing. */
  finishedAt: number | null;
}

const TOOLS: { id: Tool; label: string; icon: string }[] = [
  { id: "pen", label: "Bút", icon: "✎" },
  { id: "arrow", label: "Mũi tên", icon: "➚" },
  { id: "rect", label: "Khung", icon: "▭" },
];
const COLOR_NAMES = ["Đỏ", "Vàng", "Xanh lá", "Xanh dương", "Trắng"];
const WIDTH_NAMES = ["Mảnh", "Vừa", "Đậm"];
/** Strokes are sent to the core at most this often while drawing. */
const SEND_EVERY_MS = 16;

/**
 * The drawing layer over the recorded area. It is excluded from capture: the core draws the
 * same strokes into the video. While drawing is off the mouse passes through it and the last
 * strokes fade out.
 */
export default function DrawOverlay() {
  const canvas = useRef<HTMLCanvasElement>(null);
  const shown = useRef<Shown[]>([]);
  const current = useRef<Shown | null>(null);
  const lastSent = useRef(0);
  const holdMs = useRef(3000);
  const [on, setOn] = useState(false);
  const [tool, setTool] = useState<Tool>("pen");
  const [color, setColor] = useState(0);
  const [width, setWidth] = useState(1);
  const [message, setMessage] = useState("");

  useEffect(() => {
    drawContext()
      .then((c) => {
        setOn(c.on);
        holdMs.current = c.hold_ms;
      })
      .catch(() => {});
    const off = onDrawMode(setOn);
    return () => void off.then((u) => u());
  }, []);

  // Paint every animation frame: strokes fade on screen exactly as in the video.
  useEffect(() => {
    let frame = 0;
    const paint = () => {
      const c = canvas.current;
      const ctx = c?.getContext("2d");
      if (c && ctx) {
        const w = Math.round(window.innerWidth * devicePixelRatio);
        const h = Math.round(window.innerHeight * devicePixelRatio);
        if (c.width !== w || c.height !== h) {
          c.width = w;
          c.height = h;
        }
        ctx.clearRect(0, 0, w, h);
        const now = performance.now();
        shown.current = shown.current.filter((s) => alphaAt(s.finishedAt, now, holdMs.current) > 0);
        for (const s of shown.current) drawOne(ctx, s.stroke, alphaAt(s.finishedAt, now, holdMs.current));
      }
      frame = requestAnimationFrame(paint);
    };
    frame = requestAnimationFrame(paint);
    return () => cancelAnimationFrame(frame);
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") drawSet(false).catch(() => {});
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const send = (s: Stroke, force: boolean) => {
    const now = performance.now();
    if (!force && now - lastSent.current < SEND_EVERY_MS) return;
    lastSent.current = now;
    drawStroke(s).catch((e) => setMessage(errorMessage(e)));
  };

  const at = (e: React.PointerEvent): [number, number] => [e.clientX * devicePixelRatio, e.clientY * devicePixelRatio];

  const down = (e: React.PointerEvent) => {
    if (!on || e.button !== 0) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    const p = at(e);
    const stroke: Stroke = {
      id: Date.now() * 100 + Math.floor(Math.random() * 100),
      tool,
      color: COLORS[color],
      width: WIDTHS[width],
      points: [p],
      finished_ms: null,
    };
    current.current = { stroke, finishedAt: null };
    shown.current = [...shown.current, current.current];
    send(stroke, true);
  };

  const move = (e: React.PointerEvent) => {
    const cur = current.current;
    if (!cur) return;
    const p = at(e);
    const pts = cur.stroke.points;
    cur.stroke = { ...cur.stroke, points: cur.stroke.tool === "pen" ? [...pts, p] : [pts[0], p] };
    send(cur.stroke, false);
  };

  const up = () => {
    const cur = current.current;
    if (!cur) return;
    current.current = null;
    cur.finishedAt = performance.now();
    cur.stroke = { ...cur.stroke, finished_ms: 0 };
    send(cur.stroke, true);
  };

  const clearAll = () => {
    shown.current = [];
    drawClear().catch(() => {});
  };

  return (
    <div className={"draw" + (on ? " draw-on" : "")} onPointerDown={down} onPointerMove={move} onPointerUp={up} onPointerCancel={up}>
      <canvas ref={canvas} className="draw-canvas" />
      {on && (
        <div className="draw-bar" role="toolbar" aria-label="Công cụ vẽ" onPointerDown={(e) => e.stopPropagation()}>
          {TOOLS.map((t) => (
            <button key={t.id} className={"bar-btn bar-icon" + (tool === t.id ? " on" : "")} title={t.label} aria-label={t.label} aria-pressed={tool === t.id} onClick={() => setTool(t.id)}>
              {t.icon}
            </button>
          ))}
          <span className="bar-sep" />
          {COLORS.map((c, i) => (
            <button
              key={i}
              className={"draw-swatch" + (color === i ? " on" : "")}
              style={{ background: `rgb(${c[0]},${c[1]},${c[2]})` }}
              title={COLOR_NAMES[i]}
              aria-label={COLOR_NAMES[i]}
              aria-pressed={color === i}
              onClick={() => setColor(i)}
            />
          ))}
          <span className="bar-sep" />
          {WIDTHS.map((w, i) => (
            <button key={w} className={"bar-btn bar-icon" + (width === i ? " on" : "")} title={WIDTH_NAMES[i]} aria-label={`Nét ${WIDTH_NAMES[i].toLowerCase()}`} aria-pressed={width === i} onClick={() => setWidth(i)}>
              <span className="draw-dot" style={{ width: 4 + i * 4, height: 4 + i * 4 }} />
            </button>
          ))}
          <span className="bar-sep" />
          <button className="bar-btn" onClick={clearAll}>Xóa hết</button>
          <button className="bar-btn" title="Thoát chế độ vẽ (Esc)" onClick={() => drawSet(false).catch(() => {})}>Thoát</button>
          {message && <span className="bar-msg" role="status">{message}</span>}
        </div>
      )}
    </div>
  );
}

function drawOne(ctx: CanvasRenderingContext2D, s: Stroke, alpha: number) {
  ctx.save();
  ctx.globalAlpha = alpha;
  ctx.strokeStyle = `rgb(${s.color[0]},${s.color[1]},${s.color[2]})`;
  ctx.lineWidth = s.width;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  // One path per stroke, so a fading stroke has no darker joints (as in the video).
  ctx.beginPath();
  for (const [a, b] of segments(s)) {
    ctx.moveTo(a[0], a[1]);
    ctx.lineTo(b[0], b[1]);
  }
  ctx.stroke();
  ctx.restore();
}
