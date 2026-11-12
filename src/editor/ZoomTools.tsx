import { useEffect, useRef, useState } from "react";
import type { ZoomTool } from "./ZoomTrack";

const TOOLS: { tool: ZoomTool; label: string; title: string }[] = [
  { tool: "manual", label: "+ Zoom tay", title: "Phím Z. Bấm để đặt tại vạch phát, hoặc kéo xuống rãnh Zoom: một đoạn zoom có khung để chỉnh tay" },
  { tool: "auto", label: "+ Tự zoom theo click", title: "Bấm để đặt tại vạch phát, hoặc kéo xuống rãnh Zoom: trong vùng này zoom tự đi theo click chuột" },
];

/** Moving the pointer this far turns a press into a drag. */
const DRAG_PX = 5;

type Drag = { tool: ZoomTool; x: number; y: number; dragging: boolean };

/**
 * The Zoom tab's two tools. A click places one at the playhead; dragging one and letting go over
 * the Zoom track places it there. Plain pointer events (not HTML drag and drop, which the app
 * window keeps for files dropped from outside).
 */
export default function ZoomTools(props: { durationMs: number; autoEnabled: boolean; onPlace: (tool: ZoomTool, ms: number | null) => void }) {
  const drag = useRef<Drag | null>(null);
  const [ghost, setGhost] = useState<{ tool: ZoomTool; x: number; y: number } | null>(null);
  const live = useRef(props);
  live.current = props;

  useEffect(() => {
    const move = (e: PointerEvent) => {
      const g = drag.current;
      if (!g) return;
      if (!g.dragging && Math.hypot(e.clientX - g.x, e.clientY - g.y) < DRAG_PX) return;
      g.dragging = true;
      setGhost({ tool: g.tool, x: e.clientX, y: e.clientY });
    };
    const up = (e: PointerEvent) => {
      const g = drag.current;
      drag.current = null;
      setGhost(null);
      if (!g) return;
      if (!g.dragging) {
        live.current.onPlace(g.tool, null);
        return;
      }
      const track = document.elementFromPoint(e.clientX, e.clientY)?.closest(".ztrack");
      if (!track) return;
      const r = track.getBoundingClientRect();
      if (r.width <= 0) return;
      const f = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
      live.current.onPlace(g.tool, Math.round(f * live.current.durationMs));
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
  }, []);

  return (
    <div className="row ztools">
      {TOOLS.map((t) => (
        <button
          key={t.tool}
          className={`btn btn-small ztool ztool-${t.tool}`}
          disabled={t.tool === "auto" && !props.autoEnabled}
          title={t.title}
          onPointerDown={(e) => {
            if (e.button !== 0) return;
            e.preventDefault();
            drag.current = { tool: t.tool, x: e.clientX, y: e.clientY, dragging: false };
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              props.onPlace(t.tool, null);
            }
          }}
        >
          {t.label}
        </button>
      ))}
      {ghost && (
        <div className={`ztool-ghost ztool-${ghost.tool}`} style={{ left: ghost.x + 10, top: ghost.y + 10 }}>
          {ghost.tool === "manual" ? "Zoom tay" : "Tự zoom theo click"} — thả xuống rãnh Zoom
        </div>
      )}
    </div>
  );
}
