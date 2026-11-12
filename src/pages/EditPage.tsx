import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage, formatElapsed } from "../api";
import {
  addCut,
  editKeep,
  editLayout,
  editOpen,
  editSave,
  zoomAuto,
  zoomTrack,
  type Camera,
  type EditDoc,
  type EditSession,
  type FrameDoc,
  type KeepInfo,
  type Layout,
  type Level,
  type Span,
  type Speed,
  type Spot,
  type ZoomStyle,
  type ZoomSegment,
} from "../editApi";
import ExportPanel from "../editor/ExportPanel";
import SubtitlesPanel from "../editor/SubtitlesPanel";
import { chipAbove, cueAt, layoutSubtitle, paintSubtitle } from "../editor/subsMath";
import Preview, { type PreviewHandle } from "../editor/Preview";
import Timeline from "../editor/Timeline";
import { cursorAt, drawEffects, drawSpot } from "../editor/drawEffects";
import ZoomTrack, { type ZoomTool } from "../editor/ZoomTrack";
import ZoomTools from "../editor/ZoomTools";
import { addKey, addRegionAt, addSegmentAt, cameraAt, editFrameAt, frameAt, setActiveScale, spotAt, WHOLE } from "../editor/zoomMath";

type Tab = "cut" | "effects" | "zoom" | "frame" | "subs" | "export";
const TABS: { id: Tab; label: string }[] = [
  { id: "cut", label: "Cắt" },
  { id: "effects", label: "Hiệu ứng" },
  { id: "zoom", label: "Zoom" },
  { id: "frame", label: "Khung" },
  { id: "subs", label: "Phụ đề" },
  { id: "export", label: "Xuất" },
];

const HISTORY_LIMIT = 100;
const SAVE_DELAY_MS = 1000;
const nameOf = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** The edit page of one video: preview, timeline, tabs. Every change is saved on its own. */
export default function EditPage(props: { path: string; onClose: () => void }) {
  const [session, setSession] = useState<EditSession | null>(null);
  const [loadError, setLoadError] = useState("");
  const [doc, setDocState] = useState<EditDoc | null>(null);
  const [keep, setKeep] = useState<KeepInfo | null>(null);
  const [playhead, setPlayhead] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [selection, setSelection] = useState<Span | null>(null);
  const [tab, setTab] = useState<Tab>("cut");
  const [message, setMessage] = useState("");
  const [track, setTrack] = useState<[number, Camera, Spot][]>([]);
  const [layout, setLayout] = useState<Layout | null>(null);
  const [zoomSel, setZoomSel] = useState<number | null>(null);
  const [keySel, setKeySel] = useState<number | null>(null);
  const [regionSel, setRegionSel] = useState<number | null>(null);
  const preview = useRef<PreviewHandle>(null);
  const past = useRef<EditDoc[]>([]);
  const future = useRef<EditDoc[]>([]);
  const dirty = useRef(false);
  const saveTimer = useRef<number | undefined>(undefined);
  const latest = useRef<EditDoc | null>(null);
  latest.current = doc;

  const duration = session?.info.duration_ms ?? 0;
  const frameMs = 1000 / Math.max(1, session?.info.fps ?? 30);

  useEffect(() => {
    let alive = true;
    editOpen(props.path)
      .then((s) => {
        if (!alive) return;
        setSession(s);
        setDocState(s.doc);
        if (s.warning) setMessage(s.warning);
      })
      .catch((e) => alive && setLoadError(errorMessage(e)));
    return () => {
      alive = false;
    };
  }, [props.path]);

  const save = useCallback(async () => {
    window.clearTimeout(saveTimer.current);
    if (!dirty.current || !latest.current) return;
    dirty.current = false;
    try {
      await editSave(props.path, latest.current);
    } catch (e) {
      dirty.current = true;
      setMessage(errorMessage(e));
    }
  }, [props.path]);

  // Save whatever is pending when the page closes.
  useEffect(() => () => void save(), [save]);

  // What the edit keeps, worked out by the core.
  useEffect(() => {
    if (!doc || !session) return;
    let alive = true;
    editKeep(doc, session.info.duration_ms)
      .then((k) => alive && setKeep(k))
      .catch((e) => alive && setMessage(errorMessage(e)));
    return () => {
      alive = false;
    };
  }, [doc, session]);

  // The output frame layout, worked out by the core.
  useEffect(() => {
    if (!doc || !session) return;
    let alive = true;
    editLayout(doc, frameOf(session))
      .then((l) => alive && setLayout(l))
      .catch((e) => alive && setMessage(errorMessage(e)));
    return () => {
      alive = false;
    };
  }, [doc?.frame, session]);

  // The zoom camera path, worked out by the core, refreshed shortly after zoom edits.
  useEffect(() => {
    if (!doc || !session) return;
    if (!doc.zoom.enabled) {
      setTrack([]);
      return;
    }
    let alive = true;
    const timer = window.setTimeout(() => {
      zoomTrack(props.path, doc, frameOf(session))
        .then((t) => alive && setTrack(t))
        .catch((e) => alive && setMessage(errorMessage(e)));
    }, 150);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [doc?.zoom, session, props.path]);

  // Automatic segments follow the regions and the level: recomputed shortly after they change,
  // as part of the same undo step. Not on opening (the file already has them).
  const autoKey = doc ? JSON.stringify([doc.zoom.regions, doc.zoom.level]) : "";
  const lastAutoKey = useRef<string | null>(null);
  useEffect(() => {
    if (!doc || !session) return;
    if (lastAutoKey.current === null || lastAutoKey.current === autoKey) {
      lastAutoKey.current = autoKey;
      return;
    }
    lastAutoKey.current = autoKey;
    if (!session.effects) return;
    let alive = true;
    const timer = window.setTimeout(() => {
      const d = latest.current;
      if (!d) return;
      zoomAuto(props.path, d, frameOf(session))
        .then((segments) => {
          const now = latest.current;
          if (!alive || !now || JSON.stringify(now.zoom.segments) === JSON.stringify(segments)) return;
          changeRef.current({ ...now, zoom: { ...now.zoom, segments } }, true);
        })
        .catch((e) => alive && setMessage(errorMessage(e)));
    }, 200);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [autoKey, session, props.path]);

  /** Applies a change: recorded for undo (unless it continues the last one), saved a moment later. */
  const change = useCallback(
    (next: EditDoc, merge = false) => {
      const prev = latest.current;
      if (!prev) return;
      if (!merge || past.current.length === 0) past.current = [...past.current, prev].slice(-HISTORY_LIMIT);
      future.current = [];
      setDocState(next);
      dirty.current = true;
      window.clearTimeout(saveTimer.current);
      saveTimer.current = window.setTimeout(() => void save(), SAVE_DELAY_MS);
    },
    [save],
  );

  const changeRef = useRef(change);
  changeRef.current = change;

  const undoRedo = useCallback(
    (back: boolean) => {
      const from = back ? past : future;
      const to = back ? future : past;
      const cur = latest.current;
      const target = from.current[from.current.length - 1];
      if (!cur || !target) return;
      from.current = from.current.slice(0, -1);
      to.current = [...to.current, cur];
      setDocState(target);
      setZoomSel(null);
      dirty.current = true;
      window.clearTimeout(saveTimer.current);
      saveTimer.current = window.setTimeout(() => void save(), SAVE_DELAY_MS);
    },
    [save],
  );

  const trimEnd = doc?.trim.end_ms ?? duration;
  const removeSelection = useCallback(() => {
    if (!doc || !selection) return;
    change({ ...doc, cuts: addCut(doc.cuts, selection) });
    setSelection(null);
  }, [doc, selection, change]);

  // Page shortcuts (spec 4.1). Typing in a field never triggers them.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) return;
      const p = preview.current;
      const ctrl = e.ctrlKey || e.metaKey;
      if (ctrl && e.key.toLowerCase() === "z") {
        e.preventDefault();
        undoRedo(!e.shiftKey);
      } else if (ctrl && e.key.toLowerCase() === "y") {
        e.preventDefault();
        undoRedo(false);
      } else if (e.key === " " && !ctrl) {
        e.preventDefault();
        p?.toggle();
      } else if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
        e.preventDefault();
        const step = e.shiftKey ? 1000 : frameMs;
        p?.pause();
        p?.seek(Math.min(Math.max(playhead + (e.key === "ArrowLeft" ? -step : step), 0), duration));
      } else if (e.key.toLowerCase() === "i" && !ctrl) {
        setSelection((s) => ({ start_ms: playhead, end_ms: Math.max(s?.end_ms ?? trimEnd, playhead + frameMs) }));
      } else if (e.key.toLowerCase() === "o" && !ctrl) {
        setSelection((s) => ({ start_ms: Math.min(s?.start_ms ?? doc?.trim.start_ms ?? 0, playhead - frameMs), end_ms: playhead }));
      } else if (e.key.toLowerCase() === "z" && !ctrl && !e.altKey) {
        e.preventDefault();
        addZoomHere();
      } else if (e.key.toLowerCase() === "k" && !ctrl && !e.altKey) {
        e.preventDefault();
        addKeyHere();
      } else if (e.key === "Delete") {
        e.preventDefault();
        // A cut selection wins; then the selected keyframe; then the selected zoom segment.
        if (selection) removeSelection();
        else if (regionSel !== null && doc?.zoom.enabled && doc.zoom.regions[regionSel]) deleteRegion();
        else if (zoomSel !== null && doc?.zoom.enabled && doc.zoom.segments[zoomSel]) {
          if (keySel !== null && doc.zoom.segments[zoomSel].keys[keySel]) deleteKey();
          else {
            change({ ...doc, zoom: { ...doc.zoom, segments: doc.zoom.segments.filter((_, i) => i !== zoomSel) } });
            setZoomSel(null);
            setKeySel(null);
          }
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  /** Changes segment `index` (made manual). `merge` continues the last undo step. */
  const updateSeg = (index: number, f: (s: ZoomSegment) => ZoomSegment, merge = false) => {
    const d = latest.current;
    if (!d || !d.zoom.segments[index]) return;
    const segments = d.zoom.segments.map((s, i) => (i === index ? { ...f(s), manual: true } : s));
    change({ ...d, zoom: { ...d.zoom, segments } }, merge);
  };
  // Moving the scale slider is one undo step, not one per tick.
  const scaleRun = useRef<{ index: number; at: number } | null>(null);
  const slideScale = (index: number, scale: number) => {
    const now = performance.now();
    const merge = scaleRun.current?.index === index && now - scaleRun.current.at < 1000;
    scaleRun.current = { index, at: now };
    updateSeg(index, (s) => setActiveScale(s, playhead, scale, cameraNow()), merge);
  };

  /** Where the zoom camera is at the playhead. */
  const cameraNow = () => (track.length > 0 ? cameraAt(track, playhead) : WHOLE);

  /** Z: a new zoom segment at the playhead, centred on the mouse there (when logged). */
  const addZoomHere = (at: number = playhead) => {
    const d = latest.current;
    if (!d || !session) return;
    const p = session.effects ? cursorAt(session.effects, at) : null;
    const center: [number, number] | null = p ? [p[0] / session.info.width, p[1] / session.info.height] : null;
    const r = addSegmentAt(d.zoom.segments, at, duration, center);
    if (!r) {
      setMessage("Không thêm được đoạn zoom ở đây: chỗ này đã có đoạn zoom, hoặc còn chưa tới 0,5 giây trước đoạn sau hay cuối video.");
      return;
    }
    preview.current?.pause();
    if (Math.abs(at - playhead) > 1) preview.current?.seek(at);
    change({ ...d, zoom: { ...d.zoom, enabled: true, segments: r.segments } });
    setZoomSel(r.index);
    setKeySel(null);
    setRegionSel(null);
    setTab("zoom");
  };

  /** An automatic region at `at`: inside it, zoom follows the clicks. */
  const addAutoHere = (at: number = playhead) => {
    const d = latest.current;
    if (!d || !session) return;
    if (!session.effects) {
      setMessage("Video này không có nhật ký thao tác nên không tự zoom theo click được.");
      return;
    }
    const r = addRegionAt(d.zoom.regions, at, duration);
    if (!r) {
      setMessage("Không đặt được vùng tự động ở đây: chỗ này đã có vùng, hoặc còn chưa tới 1 giây trước cuối video.");
      return;
    }
    change({ ...d, zoom: { ...d.zoom, enabled: true, regions: r.regions } });
    setRegionSel(r.index);
    setZoomSel(null);
    setKeySel(null);
    setTab("zoom");
  };

  const deleteRegion = () => {
    const d = latest.current;
    if (!d || regionSel === null) return;
    change({ ...d, zoom: { ...d.zoom, regions: d.zoom.regions.filter((_, i) => i !== regionSel) } });
    setRegionSel(null);
  };

  const dropTool = (tool: ZoomTool, at: number) => (tool === "manual" ? addZoomHere(at) : addAutoHere(at));

  /** K: a keyframe at the playhead in the selected segment, starting where the camera is. */
  const addKeyHere = () => {
    const d = latest.current;
    const seg = d && zoomSel !== null ? d.zoom.segments[zoomSel] : null;
    if (!d || !seg || !d.zoom.enabled || zoomSel === null) {
      setMessage("Chọn một đoạn zoom trước, rồi đặt điểm lia trong đoạn đó.");
      return;
    }
    const out = addKey(seg, playhead, cameraNow());
    if (!out) {
      setMessage("Điểm lia phải nằm trong đoạn zoom đang chọn và cách các điểm khác hơn 0,05 giây.");
      return;
    }
    preview.current?.pause();
    updateSeg(zoomSel, () => out);
    setKeySel(out.keys.findIndex((k) => k.t_ms === Math.round(playhead)));
  };

  const deleteKey = () => {
    if (zoomSel === null || keySel === null) return;
    const k = keySel;
    updateSeg(zoomSel, (s) => ({ ...s, keys: s.keys.filter((_, j) => j !== k) }));
    setKeySel(null);
  };

  const setLevel = (level: Level) => {
    const d = latest.current;
    if (!d) return;
    change({ ...d, zoom: { ...d.zoom, level } });
    setZoomSel(null);
    setKeySel(null);
  };

  const clearAuto = () => {
    const d = latest.current;
    if (!d) return;
    change({ ...d, zoom: { ...d.zoom, regions: [], segments: d.zoom.segments.filter((s) => s.manual) } });
    setZoomSel(null);
    setKeySel(null);
    setRegionSel(null);
  };

  const regenerateZoom = async () => {
    const d = latest.current;
    if (!d || !session) return;
    try {
      const segments = await zoomAuto(props.path, d, frameOf(session));
      change({ ...d, zoom: { ...d.zoom, segments } });
      setZoomSel(null);
      setKeySel(null);
    } catch (e) {
      setMessage(errorMessage(e));
    }
  };

  const setZoomEnabled = async (enabled: boolean) => {
    const d = latest.current;
    if (!d || !session) return;
    // Turning zoom on adds nothing: the user drops manual zooms and automatic regions where wanted.
    if (!enabled) {
      setZoomSel(null);
      setKeySel(null);
      setRegionSel(null);
    }
    change({ ...d, zoom: { ...d.zoom, enabled } });
  };

  const close = async () => {
    await save();
    props.onClose();
  };

  if (loadError) {
    return (
      <section className="page">
        <button className="btn btn-small" onClick={props.onClose}>← Thư viện</button>
        <p className="status bar-warn" role="alert">{loadError}</p>
      </section>
    );
  }
  if (!session || !doc) {
    return (
      <section className="page">
        <p className="hint">Đang mở video…</p>
      </section>
    );
  }

  // Frame editing: the Zoom tab is open on a selected segment and the video is paused.
  const selSeg = doc.zoom.enabled && zoomSel !== null ? doc.zoom.segments[zoomSel] ?? null : null;
  const frameEditing = tab === "zoom" && !!selSeg && !playing && zoomSel !== null;

  return (
    <section className="editor">
      <header className="editor-head">
        <button className="btn btn-small" onClick={close}>← Thư viện</button>
        <span className="editor-title" title={props.path}>{nameOf(props.path)}</span>
        <span className="card-meta">
          {session.info.width}×{session.info.height} · {session.info.fps} fps · gốc {formatElapsed(duration)}
          {keep ? ` · còn ${formatElapsed(keep.output_ms)}` : ""}
        </span>
      </header>
      <div className="editor-main">
        <Preview
          ref={preview}
          path={props.path}
          srcW={session.info.width}
          srcH={session.info.height}
          layout={layout}
          keep={keep?.keep ?? []}
          onTime={setPlayhead}
          onPlaying={setPlaying}
          onError={() => setMessage("Không phát được video này trong trang chỉnh sửa.")}
          draw={(ctx, ms) => {
            // The spotlight (not while its frame is being edited), under the click marks.
            const spot = doc.zoom.enabled && !frameEditing ? spotAt(track, ms) : null;
            if (session.effects) {
              drawEffects(ctx, session.effects, ms, { ...doc.effects, keys: false }, session.info.height, ctx.canvas.height / session.info.height, undefined, spot, layout?.fill_aspect ?? null);
            } else {
              ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
              if (spot) drawSpot(ctx, spot, layout?.fill_aspect ?? null);
            }
          }}
          motionBlur={doc.zoom.motion_blur}
          drawFixed={(ctx, ms) => {
            // Shown in the preview whether or not they are burned in, so they can be checked.
            const cue = cueAt(doc.subtitles.cues, ms);
            const sub = cue ? layoutSubtitle(ctx, cue.text) : null;
            const lift = sub && doc.subtitles.burn ? chipAbove(sub, ctx.canvas.height) : undefined;
            if (session.effects) {
              drawEffects(ctx, session.effects, ms, { ...doc.effects, clicks: false }, session.info.height, ctx.canvas.height / session.info.height, lift);
            } else ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
            if (sub) paintSubtitle(ctx, sub);
          }}
          camera={doc.zoom.enabled && track.length > 0 && !frameEditing ? (ms) => cameraAt(track, ms) : undefined}
          frameEdit={
            frameEditing && selSeg && zoomSel !== null
              ? { camera: frameAt(selSeg, playhead, cameraNow()), onChange: (cam, first) => updateSeg(zoomSel, (s) => editFrameAt(s, playhead, cam), !first) }
              : undefined
          }
        />
        <aside className="editor-side">
          <div className="tabs" role="tablist">
            {TABS.map((t) => (
              <button key={t.id} role="tab" aria-selected={tab === t.id} className={"tab" + (tab === t.id ? " on" : "")} onClick={() => setTab(t.id)}>
                {t.label}
              </button>
            ))}
          </div>
          {tab === "cut" && (
            <div className="panel-body">
              <p className="hint">Kéo hai tay ở đầu và cuối dòng thời gian để bỏ phần đầu, phần cuối. Kéo trên dòng thời gian (hoặc bấm I, O) để chọn một đoạn giữa.</p>
              <button className="btn" disabled={!selection} onClick={removeSelection}>
                Bỏ đoạn này
              </button>
              {doc.cuts.length > 0 && <p className="hint">Đã bỏ {doc.cuts.length} đoạn. Bấm vào đoạn sọc để giữ lại.</p>}
              <div className="row">
                <button className="btn btn-small" disabled={past.current.length === 0} onClick={() => undoRedo(true)}>Hoàn tác</button>
                <button className="btn btn-small" disabled={future.current.length === 0} onClick={() => undoRedo(false)}>Làm lại</button>
              </div>
            </div>
          )}
          {tab === "effects" && (
            <div className="panel-body">
              {session.has_log && session.effects ? (
                <>
                  <label className="check">
                    <input
                      type="checkbox"
                      checked={doc.effects.clicks}
                      onChange={(e) => change({ ...doc, effects: { ...doc.effects, clicks: e.target.checked } })}
                    />
                    <span>
                      Vòng click
                      <span className="hint check-hint">Vòng lan ra mỗi lần bấm chuột: đỏ cam là chuột trái, xanh dương là chuột phải, xanh lá là chuột giữa. Có cả biểu tượng cuộn chuột.</span>
                    </span>
                  </label>
                  <label className="check">
                    <input
                      type="checkbox"
                      checked={doc.effects.keys}
                      onChange={(e) => change({ ...doc, effects: { ...doc.effects, keys: e.target.checked } })}
                    />
                    <span>
                      Phím tắt
                      <span className="hint check-hint">Hiện tổ hợp phím như Ctrl + S ở cạnh dưới video. Chữ bạn gõ không bao giờ hiện.</span>
                    </span>
                  </label>
                </>
              ) : (
                <p className="hint">Video này không có nhật ký thao tác, nên không có vòng click hay phím tắt.</p>
              )}
            </div>
          )}
          {tab === "zoom" && (
            <div className="panel-body">
              <label className="check">
                <input type="checkbox" checked={doc.zoom.enabled} onChange={(e) => void setZoomEnabled(e.target.checked)} />
                <span>
                  Bật zoom
                  <span className="hint check-hint">Phóng to những chỗ bạn muốn người xem chú ý. Bấm Z để thêm một đoạn tại chỗ đang phát, hoặc kéo trên rãnh Zoom dưới dòng thời gian.</span>
                </span>
              </label>
              {doc.zoom.enabled && (
                <>
                  {session.effects ? (
                    <>
                      <span className="label">Mức tự động</span>
                      <div className="row" role="radiogroup" aria-label="Mức zoom tự động">
                        {(
                          [
                            ["low", "Ít"],
                            ["normal", "Vừa"],
                            ["high", "Nhiều"],
                          ] as const
                        ).map(([v, label]) => (
                          <button key={v} className={"chip chip-toggle" + (doc.zoom.level === v ? " on" : "")} aria-pressed={doc.zoom.level === v} onClick={() => setLevel(v)}>
                            {label}
                          </button>
                        ))}
                      </div>
                      <div className="row">
                        <button className="btn btn-small" disabled={doc.zoom.regions.length === 0} onClick={() => void regenerateZoom()}>Tính lại</button>
                        <button className="btn btn-small" disabled={doc.zoom.regions.length === 0 && !doc.zoom.segments.some((s) => !s.manual)} onClick={clearAuto}>
                          Xóa vùng tự động
                        </button>
                      </div>
                      <p className="hint">Đoạn xanh là tự động (trong vùng nét đứt), đoạn vàng là bạn làm tay. Mức Ít / Vừa / Nhiều áp cho các vùng tự động.</p>
                    </>
                  ) : (
                    <p className="hint">Video này không có nhật ký thao tác: chỉ thêm zoom bằng tay được.</p>
                  )}
                  {regionSel !== null && doc.zoom.regions[regionSel] && (
                    <div className="row">
                      <span className="hint">Vùng tự động đang chọn.</span>
                      <button className="btn btn-small btn-danger" onClick={deleteRegion}>Xóa vùng này</button>
                    </div>
                  )}
                  <label className="check">
                    <input type="checkbox" checked={doc.zoom.motion_blur} onChange={(e) => change({ ...doc, zoom: { ...doc.zoom, motion_blur: e.target.checked } })} />
                    <span>
                      Nhòe chuyển động khi zoom
                      <span className="hint check-hint">Video xuất ra nhòe nhẹ lúc phóng to, thu nhỏ và lia, trông mượt như quay phim. Xuất lâu hơn một chút.</span>
                    </span>
                  </label>
                  <span className="label">Thêm vào rãnh Zoom</span>
                  <ZoomTools durationMs={duration} autoEnabled={!!session.effects} onPlace={(tool, ms) => dropTool(tool, ms ?? playhead)} />
                  <p className="hint">Kéo một nút xuống rãnh Zoom (hoặc bấm để đặt tại vạch phát). Ngoài các đoạn và vùng đã đặt, video giữ toàn màn hình.</p>
                  {selSeg && zoomSel !== null ? (
                    <ZoomControls
                      seg={selSeg}
                      frame={frameAt(selSeg, playhead, cameraNow())}
                      keySel={keySel}
                      playing={playing}
                      hasCursor={!!session.effects}
                      onScale={(scale) => slideScale(zoomSel, scale)}
                      onSpeed={(speed: Speed) => updateSeg(zoomSel, (s) => ({ ...s, speed }))}
                      onStyle={(style: ZoomStyle) => updateSeg(zoomSel, (s) => ({ ...s, style }))}
                      onAddKey={addKeyHere}
                      onDeleteKey={deleteKey}
                      onFollow={() => {
                        updateSeg(zoomSel, (s) => ({ ...s, center: null, keys: [] }));
                        setKeySel(null);
                      }}
                      onDelete={() => {
                        change({ ...doc, zoom: { ...doc.zoom, segments: doc.zoom.segments.filter((_, i) => i !== zoomSel) } });
                        setZoomSel(null);
                        setKeySel(null);
                      }}
                    />
                  ) : (
                    <p className="hint">Bấm vào một đoạn trên rãnh Zoom: video dừng và hiện khung zoom để bạn kéo tới chỗ muốn phóng.</p>
                  )}
                </>
              )}
            </div>
          )}
          {tab === "frame" && (
            <FramePanel
              frame={doc.frame}
              onChange={(patch, merge) => {
                const d = latest.current;
                if (d) change({ ...d, frame: { ...d.frame, ...patch } }, merge);
              }}
            />
          )}
          <SubtitlesPanel
            hidden={tab !== "subs"}
            path={props.path}
            doc={doc}
            durationMs={duration}
            hasAudio={session.info.has_audio}
            playhead={playhead}
            onChange={(subtitles, merge) => {
              const d = latest.current;
              if (d) change({ ...d, subtitles }, merge);
            }}
            onSeek={(ms) => {
              preview.current?.pause();
              preview.current?.seek(ms);
            }}
            beforeExport={save}
          />
          {/* Always mounted, so a running export keeps its progress and result across tabs. */}
          <ExportPanel
            hidden={tab !== "export"}
            path={props.path}
            doc={doc}
            canExport={!!keep?.can_export}
            outputMs={keep?.output_ms ?? 0}
            beforeExport={save}
          />
        </aside>
      </div>
      <div className="editor-transport">
        <button className="btn btn-small" aria-label={playing ? "Dừng" : "Phát"} onClick={() => preview.current?.toggle()}>
          {playing ? "❚❚" : "▶"}
        </button>
        <Timeline
          durationMs={duration}
          trimStart={doc.trim.start_ms}
          trimEnd={trimEnd}
          cuts={doc.cuts}
          playhead={playhead}
          selection={selection}
          onSeek={(ms) => preview.current?.seek(ms)}
          onTrim={(start, end, first) => change({ ...doc, trim: { start_ms: start, end_ms: end >= duration ? null : end } }, !first)}
          onSelect={(s) => {
            setSelection(s);
            if (s) setZoomSel(null);
          }}
          onRestoreCut={(i) => change({ ...doc, cuts: doc.cuts.filter((_, j) => j !== i) })}
        />
      </div>
      <div className="editor-transport">
        {doc.zoom.enabled && (
          <ZoomTrack
            durationMs={duration}
            segments={doc.zoom.segments}
            selected={zoomSel}
            selectedKey={keySel}
            regions={doc.zoom.regions}
            selectedRegion={regionSel}
            onSelectRegion={(i) => {
              setRegionSel(i);
              setZoomSel(null);
              setKeySel(null);
              if (i !== null) setTab("zoom");
            }}
            onRegionsChange={(regions, first) => change({ ...doc, zoom: { ...doc.zoom, regions } }, !first)}
            playhead={playhead}
            onSelect={(i) => {
              setZoomSel(i);
              setKeySel(null);
              setRegionSel(null);
              if (i === null) return;
              setTab("zoom");
              const s = doc.zoom.segments[i];
              if (s && (playhead < s.start_ms || playhead >= s.end_ms)) {
                preview.current?.pause();
                preview.current?.seek(s.start_ms);
              }
            }}
            onSelectKey={(i, k) => {
              setZoomSel(i);
              setKeySel(k);
              setTab("zoom");
            }}
            onSeek={(ms) => {
              preview.current?.pause();
              preview.current?.seek(ms);
            }}
            onChange={(segments, first) => change({ ...doc, zoom: { ...doc.zoom, segments } }, !first)}
          />
        )}
      </div>
      {message && <p className="status bar-warn editor-msg" role="alert">{message}</p>}
    </section>
  );
}

function frameOf(s: EditSession) {
  return { width: s.info.width, height: s.info.height, duration_ms: s.info.duration_ms };
}

const SPEEDS: [Speed, string][] = [
  ["slow", "Chậm"],
  ["normal", "Vừa"],
  ["fast", "Nhanh"],
  ["instant", "Tức thì"],
];

/** Controls for the selected zoom segment. */
function ZoomControls(props: {
  seg: ZoomSegment;
  /** The frame being edited at the playhead. */
  frame: Camera;
  keySel: number | null;
  playing: boolean;
  hasCursor: boolean;
  onScale: (scale: number) => void;
  onSpeed: (speed: Speed) => void;
  onStyle: (style: ZoomStyle) => void;
  onAddKey: () => void;
  onDeleteKey: () => void;
  onFollow: () => void;
  onDelete: () => void;
}) {
  const { seg, frame } = props;
  const following = !seg.center && seg.keys.length === 0;
  return (
    <div className="zoom-controls">
      <p className="hint">
        {props.playing
          ? "Dừng video để kéo khung zoom."
          : "Kéo khung trên video để dời, kéo góc để phóng to hay nhỏ. Tua tới lúc khác trong đoạn rồi chỉnh khung: một điểm ◆ được ghi ở đó và camera lia theo."}
      </p>
      <label className="zoom-scale">
        Mức phóng: <strong>{frame.scale.toFixed(1)}×</strong>
        <input type="range" min={1} max={4} step={0.1} value={frame.scale} onChange={(e) => props.onScale(Number(e.target.value))} />
      </label>
      {frame.scale > 2 && <p className="hint bar-warn">Phóng hơn 2× thì video có thể bị mờ.</p>}
      <p className="hint">
        Tâm:{" "}
        {seg.keys.length > 0
          ? `lia qua ${seg.keys.length + 1} khung (${seg.keys.length} điểm ◆)`
          : seg.center
            ? "cố định"
            : props.hasCursor
              ? "đi theo con trỏ chuột"
              : "giữa khung hình"}
      </p>
      <div className="row">
        <button className="btn btn-small" onClick={props.onAddKey} title="Phím K: thêm một điểm tại chỗ đang phát, rồi kéo khung tới chỗ mới">
          ◆ Thêm điểm
        </button>
        {props.keySel !== null && seg.keys[props.keySel] && (
          <button className="btn btn-small" onClick={props.onDeleteKey}>Xóa điểm này</button>
        )}
        {!following && (
          <button className="btn btn-small" onClick={props.onFollow}>
            {props.hasCursor ? "Theo con trỏ" : "Về giữa"}
          </button>
        )}
      </div>
      <span className="label">Kiểu</span>
      <div className="row" role="radiogroup" aria-label="Kiểu zoom">
        {(
          [
            ["zoom", "Phóng to"],
            ["spotlight", "Làm tối xung quanh"],
          ] as const
        ).map(([v, label]) => (
          <button key={v} className={"chip chip-toggle" + ((seg.style ?? "zoom") === v ? " on" : "")} aria-pressed={(seg.style ?? "zoom") === v} onClick={() => props.onStyle(v)}>
            {label}
          </button>
        ))}
      </div>
      {seg.style === "spotlight" && <p className="hint">Video giữ nguyên cỡ; vùng trong khung sáng, phần còn lại tối đi.</p>}
      <span className="label">Tốc độ</span>
      <div className="row" role="radiogroup" aria-label="Tốc độ zoom">
        {SPEEDS.map(([v, label]) => (
          <button key={v} className={"chip chip-toggle" + (seg.speed === v ? " on" : "")} aria-pressed={seg.speed === v} onClick={() => props.onSpeed(v)}>
            {label}
          </button>
        ))}
      </div>
      <button className="btn btn-small btn-danger" onClick={props.onDelete}>Xóa đoạn zoom</button>
    </div>
  );
}

const BACKGROUNDS: { id: string; label: string; css: string }[] = [
  { id: "slate", label: "Xám đậm", css: "#1e293b" },
  { id: "black", label: "Đen", css: "#000000" },
  { id: "white", label: "Trắng", css: "#ffffff" },
  { id: "ocean", label: "Biển", css: "linear-gradient(135deg, #0ea5e9, #6366f1)" },
  { id: "sunset", label: "Hoàng hôn", css: "linear-gradient(135deg, #f97316, #ec4899)" },
  { id: "forest", label: "Rừng", css: "linear-gradient(135deg, #10b981, #0f766e)" },
];

/** The "Khung" tab: output aspect, fit or fill, and the decoration around the video. */
function FramePanel(props: { frame: FrameDoc; onChange: (patch: Partial<FrameDoc>, merge: boolean) => void }) {
  const f = props.frame;
  // Dragging a slider is one undo step, not one per tick.
  const run = useRef<{ key: string; at: number } | null>(null);
  const set = (patch: Partial<FrameDoc>) => {
    run.current = null;
    props.onChange(patch, false);
  };
  const slide = (key: "padding" | "radius", value: number) => {
    const now = performance.now();
    const merge = run.current?.key === key && now - run.current.at < 1000;
    run.current = { key, at: now };
    props.onChange({ [key]: value }, merge);
  };
  const chip = (on: boolean) => "chip chip-toggle" + (on ? " on" : "");
  return (
    <div className="panel-body">
      <span className="label">Tỉ lệ</span>
      <div className="row" role="radiogroup" aria-label="Tỉ lệ khung">
        {(
          [
            ["source", "Gốc"],
            ["16:9", "16:9"],
            ["9:16", "9:16"],
            ["1:1", "1:1"],
          ] as const
        ).map(([v, label]) => (
          <button key={v} className={chip(f.aspect === v)} aria-pressed={f.aspect === v} onClick={() => set({ aspect: v })}>
            {label}
          </button>
        ))}
      </div>
      <div className="row" role="radiogroup" aria-label="Cách đặt video">
        <button className={chip(f.fit === "fit")} aria-pressed={f.fit === "fit"} onClick={() => set({ fit: "fit" })} title="Cả video nằm trong khung, chỗ trống là nền">
          Vừa khung
        </button>
        <button className={chip(f.fit === "fill")} aria-pressed={f.fit === "fill"} onClick={() => set({ fit: "fill" })} title="Cắt video cho kín khung; tâm đi theo zoom">
          Lấp đầy
        </button>
      </div>
      <span className="label">Nền</span>
      <div className="row">
        {BACKGROUNDS.map((b) => (
          <button
            key={b.id}
            className={"bg-swatch" + (f.background === b.id ? " on" : "")}
            style={{ background: b.css }}
            title={b.label}
            aria-label={`Nền ${b.label}`}
            aria-pressed={f.background === b.id}
            onClick={() => set({ background: b.id })}
          />
        ))}
      </div>
      <label className="zoom-scale">
        Khoảng đệm: <strong>{Math.round(f.padding * 100)}%</strong>
        <input type="range" min={0} max={0.2} step={0.01} value={f.padding} onChange={(e) => slide("padding", Number(e.target.value))} />
      </label>
      <label className="zoom-scale">
        Bo góc: <strong>{Math.round(f.radius)} px</strong>
        <input type="range" min={0} max={40} step={1} value={f.radius} onChange={(e) => slide("radius", Number(e.target.value))} />
      </label>
      <label className="check">
        <input type="checkbox" checked={f.shadow} onChange={(e) => set({ shadow: e.target.checked })} />
        <span>Bóng đổ</span>
      </label>
      <p className="hint">Nền, khoảng đệm, bo góc và bóng chỉ thấy khi video không phủ kín khung.</p>
    </div>
  );
}
