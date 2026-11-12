import { useRef } from "react";
import type { Camera } from "../editApi";
import { frameRect, moveFrame, resizeFrame, type Corner } from "./zoomMath";

interface Props {
  /** The frame being edited. */
  camera: Camera;
  srcW: number;
  srcH: number;
  /** Crop aspect of "Lấp đầy", or null. */
  fill: number | null;
  /** `first` is true for the first change of a drag, so one drag is one undo step. */
  onChange: (cam: Camera, first: boolean) => void;
}

type Drag = { kind: "move"; sx: number; sy: number; orig: Camera; moved: boolean } | { kind: Corner; orig: Camera; moved: boolean };

const CORNERS: Corner[] = ["nw", "ne", "sw", "se"];

/**
 * The zoom frame over the unzoomed preview: drag inside to move it, drag a corner to resize it.
 * Everything outside the frame is dimmed. It fills the video window, which shows the whole
 * source while the frame is edited (in Fill too, so the frame can go anywhere the export can).
 */
export default function ZoomFrame(props: Props) {
  const box = useRef<HTMLDivElement>(null);
  const drag = useRef<Drag | null>(null);
  const { srcW, srcH, fill } = props;
  const r = frameRect(props.camera, srcW, srcH, fill);
  const pct = (v: number) => `${v * 100}%`;

  /** Source pixel under the pointer. */
  const at = (e: React.PointerEvent): [number, number] => {
    const r = box.current?.getBoundingClientRect();
    if (!r || r.width === 0 || r.height === 0) return [0, 0];
    return [((e.clientX - r.left) / r.width) * srcW, ((e.clientY - r.top) / r.height) * srcH];
  };

  const down = (e: React.PointerEvent, kind: "move" | Corner) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    e.preventDefault();
    box.current?.setPointerCapture(e.pointerId);
    const [sx, sy] = at(e);
    drag.current = kind === "move" ? { kind, sx, sy, orig: props.camera, moved: false } : { kind, orig: props.camera, moved: false };
  };

  const move = (e: React.PointerEvent) => {
    const g = drag.current;
    if (!g) return;
    const [px, py] = at(e);
    const first = !g.moved;
    g.moved = true;
    const cam = g.kind === "move" ? moveFrame(g.orig, px - g.sx, py - g.sy, srcW, srcH, fill) : resizeFrame(g.orig, g.kind, px, py, srcW, srcH, fill);
    props.onChange(cam, first);
  };

  const up = () => {
    drag.current = null;
  };

  return (
    <div ref={box} className="zframe-layer" onPointerMove={move} onPointerUp={up} onPointerCancel={up}>
      <div
        className="zframe"
        style={{ left: pct(r.left), top: pct(r.top), width: pct(r.width), height: pct(r.height) }}
        onPointerDown={(e) => down(e, "move")}
        title="Kéo để dời vùng zoom; kéo góc để phóng to hay nhỏ"
      >
        <span className="zframe-label">{props.camera.scale.toFixed(1)}×</span>
        {CORNERS.map((c) => (
          <span key={c} className={`zframe-handle zframe-${c}`} onPointerDown={(e) => down(e, c)} aria-hidden="true" />
        ))}
      </div>
    </div>
  );
}
