import { useEffect, useMemo, useState } from "react";
import { MIN_REGION, pickerContext, pickerDone, type PickerContext, type Rect, type WindowInfo } from "../api";

interface Point {
  x: number;
  y: number;
}

/**
 * Transparent overlay covering one monitor. Region mode: drag a rectangle. Window mode: click
 * a window. Esc cancels. The overlay works in CSS pixels; results are sent in physical pixels.
 */
export default function PickerOverlay() {
  const [ctx, setCtx] = useState<PickerContext | null>(null);
  const [dragStart, setDragStart] = useState<Point | null>(null);
  const [pointer, setPointer] = useState<Point | null>(null);
  const scale = window.devicePixelRatio || 1;

  useEffect(() => {
    pickerContext().then(setCtx).catch(() => pickerDone(null));
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") pickerDone(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Window rectangles in this overlay's CSS pixels, top-most first.
  const windows = useMemo(() => {
    if (!ctx) return [];
    const { x, y } = ctx.monitor.rect;
    return ctx.windows.map((w) => ({
      info: w,
      box: { x: (w.rect.x - x) / scale, y: (w.rect.y - y) / scale, w: w.rect.w / scale, h: w.rect.h / scale },
    }));
  }, [ctx, scale]);

  if (!ctx) return <div className="picker" />;

  const toPhysical = (css: Rect): Rect => ({
    x: Math.round(css.x * scale) + ctx.monitor.rect.x,
    y: Math.round(css.y * scale) + ctx.monitor.rect.y,
    w: Math.round(css.w * scale),
    h: Math.round(css.h * scale),
  });

  if (ctx.mode === "window") {
    const hovered = pointer
      ? windows.find(({ box }) => pointer.x >= box.x && pointer.x < box.x + box.w && pointer.y >= box.y && pointer.y < box.y + box.h)
      : undefined;
    const choose = (w: WindowInfo | undefined) => {
      if (w) pickerDone({ kind: "window", hwnd: w.hwnd });
    };
    return (
      <div
        className="picker picker-window"
        onMouseMove={(e) => setPointer({ x: e.clientX, y: e.clientY })}
        onClick={() => choose(hovered?.info)}
      >
        {hovered && (
          <div
            className="picker-box"
            style={{ left: hovered.box.x, top: hovered.box.y, width: hovered.box.w, height: hovered.box.h }}
          >
            <span className="picker-tag">{hovered.info.title}</span>
          </div>
        )}
        <p className="picker-hint">Bấm vào cửa sổ muốn quay · Esc để hủy</p>
      </div>
    );
  }

  const selection: Rect | null =
    dragStart && pointer
      ? {
          x: Math.min(dragStart.x, pointer.x),
          y: Math.min(dragStart.y, pointer.y),
          w: Math.abs(pointer.x - dragStart.x),
          h: Math.abs(pointer.y - dragStart.y),
        }
      : null;
  const physical = selection ? toPhysical(selection) : null;
  const tooSmall = physical ? physical.w < MIN_REGION || physical.h < MIN_REGION : false;

  return (
    <div
      className="picker picker-region"
      onMouseDown={(e) => {
        setDragStart({ x: e.clientX, y: e.clientY });
        setPointer({ x: e.clientX, y: e.clientY });
      }}
      onMouseMove={(e) => dragStart && setPointer({ x: e.clientX, y: e.clientY })}
      onMouseUp={() => {
        if (physical && !tooSmall) {
          pickerDone({ kind: "region", monitor: ctx.monitor.index, rect: physical });
        } else {
          setDragStart(null); // too small: let the user try again
          setPointer(null);
        }
      }}
    >
      {selection && physical ? (
        <div
          className={"picker-box picker-cutout" + (tooSmall ? " picker-bad" : "")}
          style={{ left: selection.x, top: selection.y, width: selection.w, height: selection.h }}
        >
          <span className="picker-tag">
            {physical.w} × {physical.h}
            {tooSmall ? ` · tối thiểu ${MIN_REGION} px` : ""}
          </span>
        </div>
      ) : (
        <div className="picker-dim" />
      )}
      <p className="picker-hint">Kéo chuột để chọn vùng quay · Esc để hủy</p>
    </div>
  );
}
