import { useRef, useState } from "react";
import { formatPrecise, type Span } from "../editApi";

interface Props {
  durationMs: number;
  trimStart: number;
  trimEnd: number;
  cuts: Span[];
  playhead: number;
  selection: Span | null;
  onSeek: (ms: number) => void;
  /** `first` is true for the first move of a drag, so one drag is one undo step. */
  onTrim: (start: number, end: number, first: boolean) => void;
  onSelect: (span: Span | null) => void;
  onRestoreCut: (index: number) => void;
}

type Drag = { kind: "start" | "end"; moved: boolean } | { kind: "select"; from: number; moved: boolean } | { kind: "scrub" };

/** A press this close to the playhead drags the playhead instead of selecting. */
const HEAD_GRAB_PX = 8;

/** Shortest selection made by dragging; anything less is a click that seeks. */
const MIN_DRAG_MS = 100;

/**
 * The Video track: the whole source video, trim handles at both ends, removed spans striped,
 * a selection made by dragging, and the playhead.
 */
export default function Timeline(props: Props) {
  const track = useRef<HTMLDivElement>(null);
  const drag = useRef<Drag | null>(null);
  const [openCut, setOpenCut] = useState<number | null>(null);
  const d = Math.max(1, props.durationMs);
  const pct = (ms: number) => `${(Math.min(Math.max(ms, 0), d) / d) * 100}%`;

  const msAt = (clientX: number) => {
    const r = track.current?.getBoundingClientRect();
    if (!r || r.width === 0) return 0;
    return Math.round(Math.min(Math.max((clientX - r.left) / r.width, 0), 1) * d);
  };

  const onDown = (e: React.PointerEvent, kind: "start" | "end" | "track") => {
    if (e.button !== 0) return;
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    setOpenCut(null);
    if (kind === "track") {
      const r = track.current?.getBoundingClientRect();
      const headX = r ? r.left + (Math.min(Math.max(props.playhead, 0), d) / d) * r.width : -1e9;
      if (Math.abs(e.clientX - headX) <= HEAD_GRAB_PX) {
        drag.current = { kind: "scrub" };
        return;
      }
    }
    drag.current = kind === "track" ? { kind: "select", from: msAt(e.clientX), moved: false } : { kind, moved: false };
  };

  const onMove = (e: React.PointerEvent) => {
    const g = drag.current;
    if (!g) return;
    const ms = msAt(e.clientX);
    if (g.kind === "scrub") {
      props.onSeek(ms);
      return;
    }
    if (g.kind === "start" || g.kind === "end") {
      const first = !g.moved;
      g.moved = true;
      if (g.kind === "start") props.onTrim(Math.max(0, Math.min(ms, props.trimEnd - MIN_DRAG_MS)), props.trimEnd, first);
      else props.onTrim(props.trimStart, Math.min(d, Math.max(ms, props.trimStart + MIN_DRAG_MS)), first);
    } else if (g.kind === "select" && Math.abs(ms - g.from) >= MIN_DRAG_MS) {
      g.moved = true;
      props.onSelect({ start_ms: Math.min(g.from, ms), end_ms: Math.max(g.from, ms) });
    }
  };

  const onUp = (e: React.PointerEvent) => {
    const g = drag.current;
    drag.current = null;
    if (g?.kind === "select" && !g.moved) {
      props.onSelect(null);
      props.onSeek(msAt(e.clientX));
    }
  };

  return (
    <div className="timeline">
      <div className="timeline-labels">
        <span>{formatPrecise(props.trimStart)}</span>
        <span className="timeline-now">{formatPrecise(props.playhead)}</span>
        <span>{formatPrecise(props.trimEnd)}</span>
      </div>
      <div
        ref={track}
        className="track"
        role="slider"
        aria-label="Dòng thời gian video"
        aria-valuemin={0}
        aria-valuemax={d}
        aria-valuenow={Math.round(props.playhead)}
        aria-valuetext={formatPrecise(props.playhead)}
        onPointerDown={(e) => onDown(e, "track")}
        onPointerMove={onMove}
        onPointerUp={onUp}
      >
        <div className="track-out" style={{ left: 0, width: pct(props.trimStart) }} />
        <div className="track-out" style={{ left: pct(props.trimEnd), right: 0 }} />
        {props.cuts.map((c, i) => (
          <div
            key={`${c.start_ms}-${c.end_ms}`}
            className="track-cut"
            style={{ left: pct(c.start_ms), width: `calc(${pct(c.end_ms)} - ${pct(c.start_ms)})` }}
            title="Đoạn đã bỏ — bấm để giữ lại"
            onPointerDown={(e) => {
              e.stopPropagation();
              setOpenCut(openCut === i ? null : i);
            }}
          >
            {openCut === i && (
              <button
                className="btn btn-small track-pop"
                onPointerDown={(e) => e.stopPropagation()}
                onClick={() => {
                  setOpenCut(null);
                  props.onRestoreCut(i);
                }}
              >
                Giữ lại
              </button>
            )}
          </div>
        ))}
        {props.selection && (
          <div
            className="track-sel"
            style={{ left: pct(props.selection.start_ms), width: `calc(${pct(props.selection.end_ms)} - ${pct(props.selection.start_ms)})` }}
          />
        )}
        <div
          className="track-handle"
          style={{ left: pct(props.trimStart) }}
          title="Kéo để cắt phần đầu"
          onPointerDown={(e) => onDown(e, "start")}
          onPointerMove={onMove}
          onPointerUp={onUp}
        />
        <div
          className="track-handle track-handle-end"
          style={{ left: pct(props.trimEnd) }}
          title="Kéo để cắt phần cuối"
          onPointerDown={(e) => onDown(e, "end")}
          onPointerMove={onMove}
          onPointerUp={onUp}
        />
        <div className="track-head" style={{ left: pct(props.playhead) }}>
          <span
            className="track-knob"
            title="Kéo để tua"
            onPointerDown={(e) => {
              if (e.button !== 0) return;
              e.stopPropagation();
              track.current?.setPointerCapture(e.pointerId);
              setOpenCut(null);
              drag.current = { kind: "scrub" };
            }}
          />
        </div>
      </div>
    </div>
  );
}
