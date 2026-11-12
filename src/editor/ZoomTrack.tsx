import { useRef } from "react";
import type { Span, ZoomSegment } from "../editApi";
import { grabKind, keepKeysInside, moveSegment } from "./zoomMath";

interface Props {
  durationMs: number;
  segments: ZoomSegment[];
  selected: number | null;
  playhead: number;
  onSelect: (index: number | null) => void;
  /** The selected keyframe of the selected segment. */
  selectedKey: number | null;
  onSelectKey: (segment: number, key: number) => void;
  onSeek: (ms: number) => void;
  /** `first` is true for the first change of a drag, so one drag is one undo step. */
  onChange: (segments: ZoomSegment[], first: boolean) => void;
  /** Automatic regions: inside them zoom follows the clicks. */
  regions: Span[];
  selectedRegion: number | null;
  onSelectRegion: (index: number | null) => void;
  onRegionsChange: (regions: Span[], first: boolean) => void;
}

export type ZoomTool = "manual" | "auto";

type Drag =
  | { kind: "start" | "end" | "move"; index: number; grabMs: number; orig: ZoomSegment; moved: boolean }
  | { kind: "create"; fromMs: number; index: number | null }
  | { kind: "key"; index: number; key: number; orig: ZoomSegment; moved: boolean }
  | { kind: "region"; part: "start" | "end" | "move"; index: number; grabMs: number; orig: Span; moved: boolean };

/** An automatic region is at least this long. */
const REGION_MIN_MS = 1000;

const MIN_LEN_MS = 200;
/** Keyframes stay this far from each other and from the segment's ends. */
const KEY_GAP = 51;

/**
 * The Zoom track under the video track: one block per zoom segment. Drag an edge to change
 * its times, drag the middle to move it, drag on empty space to add a manual segment.
 */
export default function ZoomTrack(props: Props) {
  const track = useRef<HTMLDivElement>(null);
  const drag = useRef<Drag | null>(null);
  const d = Math.max(1, props.durationMs);
  const pct = (ms: number) => `${(Math.min(Math.max(ms, 0), d) / d) * 100}%`;

  const msAt = (clientX: number) => {
    const r = track.current?.getBoundingClientRect();
    if (!r || r.width === 0) return 0;
    return Math.round(Math.min(Math.max((clientX - r.left) / r.width, 0), 1) * d);
  };

  const replace = (index: number, seg: ZoomSegment, first: boolean) => {
    const next = props.segments.slice();
    next[index] = seg;
    props.onChange(next, first);
  };

  const downOnSegment = (e: React.PointerEvent, index: number) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    track.current?.setPointerCapture(e.pointerId);
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const kind = grabKind(e.clientX - r.left, r.width);
    drag.current = { kind, index, grabMs: msAt(e.clientX), orig: props.segments[index], moved: false };
    props.onSelect(index);
  };

  const downOnRegion = (e: React.PointerEvent, index: number) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    track.current?.setPointerCapture(e.pointerId);
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    drag.current = { kind: "region", part: grabKind(e.clientX - r.left, r.width), index, grabMs: msAt(e.clientX), orig: props.regions[index], moved: false };
    props.onSelectRegion(index);
  };

  const downOnKey = (e: React.PointerEvent, index: number, key: number) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    track.current?.setPointerCapture(e.pointerId);
    drag.current = { kind: "key", index, key, orig: props.segments[index], moved: false };
    props.onSelectKey(index, key);
    props.onSeek(props.segments[index].keys[key].t_ms);
  };

  const downOnTrack = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    track.current?.setPointerCapture(e.pointerId);
    drag.current = { kind: "create", fromMs: msAt(e.clientX), index: null };
  };

  const move = (e: React.PointerEvent) => {
    const g = drag.current;
    if (!g) return;
    const ms = msAt(e.clientX);
    if (g.kind === "create") {
      if (Math.abs(ms - g.fromMs) < MIN_LEN_MS) return;
      const seg: ZoomSegment = { start_ms: Math.min(g.fromMs, ms), end_ms: Math.max(g.fromMs, ms), scale: 1.5, center: null, manual: true, keys: [], speed: "normal", style: "zoom" };
      if (g.index === null) {
        g.index = props.segments.length;
        props.onChange([...props.segments, seg], true);
        props.onSelect(g.index);
      } else {
        replace(g.index, seg, false);
      }
      return;
    }
    const first = !g.moved;
    g.moved = true;
    if (g.kind === "region") {
      const o = g.orig;
      const len = o.end_ms - o.start_ms;
      let r: Span;
      if (g.part === "start") r = { start_ms: Math.min(Math.max(0, ms), o.end_ms - REGION_MIN_MS), end_ms: o.end_ms };
      else if (g.part === "end") r = { start_ms: o.start_ms, end_ms: Math.max(Math.min(d, ms), o.start_ms + REGION_MIN_MS) };
      else {
        const start = Math.min(Math.max(0, o.start_ms + ms - g.grabMs), d - len);
        r = { start_ms: start, end_ms: start + len };
      }
      const next = props.regions.slice();
      next[g.index] = r;
      props.onRegionsChange(next, first);
      return;
    }
    const o = g.orig;
    if (g.kind === "key") {
      // A keyframe moves between its neighbours, inside its segment.
      const lo = (g.key > 0 ? o.keys[g.key - 1].t_ms : o.start_ms) + KEY_GAP;
      const hi = (g.key < o.keys.length - 1 ? o.keys[g.key + 1].t_ms : o.end_ms) - KEY_GAP;
      if (hi < lo) return;
      const t = Math.min(Math.max(ms, lo), hi);
      replace(g.index, { ...o, keys: o.keys.map((k, j) => (j === g.key ? { ...k, t_ms: t } : k)), manual: true }, first);
      props.onSeek(t);
      return;
    }
    const len = o.end_ms - o.start_ms;
    let seg: ZoomSegment;
    // Dragging an edge drops the keyframes it passes; moving the segment carries them along.
    if (g.kind === "start") seg = keepKeysInside({ ...o, start_ms: Math.min(Math.max(0, ms), o.end_ms - MIN_LEN_MS) });
    else if (g.kind === "end") seg = keepKeysInside({ ...o, end_ms: Math.max(Math.min(d, ms), o.start_ms + MIN_LEN_MS) });
    else {
      const start = Math.min(Math.max(0, o.start_ms + ms - g.grabMs), d - len);
      seg = moveSegment(o, start - o.start_ms);
    }
    replace(g.index, { ...seg, manual: true }, first);
  };

  const up = () => {
    const g = drag.current;
    drag.current = null;
    // A plain click on empty track: no segment, the playhead goes there.
    if (g?.kind === "create" && g.index === null) {
      props.onSelect(null);
      props.onSeek(g.fromMs);
    }
  };

  return (
    <div className="ztrack-row">
      <span className="ztrack-label">Zoom</span>
      <div
        ref={track}
        className="track ztrack"
        onPointerDown={downOnTrack}
        onPointerMove={move}
        onPointerUp={up}
        title="Bấm để tua tới đó; kéo trên chỗ trống để thêm một đoạn zoom; thả nút từ thẻ Zoom xuống đây"
      >
        {props.regions.map((r, i) => (
          <div
            key={`r${i}`}
            className={"zregion" + (i === props.selectedRegion ? " on" : "")}
            style={{ left: pct(r.start_ms), width: `calc(${pct(r.end_ms)} - ${pct(r.start_ms)})` }}
            onPointerDown={(e) => downOnRegion(e, i)}
            title="Vùng tự zoom theo click — kéo mép để đổi, kéo giữa để dời, Delete để xóa"
          >
            <span className="zregion-text">Tự động</span>
          </div>
        ))}
        {props.segments.map((s, i) => (
          <div
            key={i}
            className={"zseg" + (i === props.selected ? " on" : "") + (s.manual ? " manual" : "")}
            style={{ left: pct(s.start_ms), width: `calc(${pct(s.end_ms)} - ${pct(s.start_ms)})` }}
            onPointerDown={(e) => downOnSegment(e, i)}
            title={`Zoom ${s.scale.toFixed(1)}× — kéo mép để đổi thời gian, kéo giữa để dời`}
          >
            <span className="zseg-text">{s.style === "spotlight" ? "Spot" : `${s.scale.toFixed(1)}×`}</span>
          </div>
        ))}
        {props.segments.flatMap((s, i) =>
          s.keys.map((k, j) => (
            <div
              key={`k${i}-${j}`}
              className={"zkey" + (i === props.selected && j === props.selectedKey ? " on" : "")}
              style={{ left: pct(k.t_ms) }}
              onPointerDown={(e) => downOnKey(e, i, j)}
              title={`Điểm lia ${k.scale.toFixed(1)}× — kéo để đổi giờ, Delete để xóa`}
            />
          )),
        )}
        <div className="track-head" style={{ left: pct(props.playhead) }} />
      </div>
    </div>
  );
}
