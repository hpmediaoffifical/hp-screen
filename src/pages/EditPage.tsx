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
  type Span,
  type ZoomSegment,
} from "../editApi";
import ExportPanel from "../editor/ExportPanel";
import Preview, { type PreviewHandle } from "../editor/Preview";
import Timeline from "../editor/Timeline";
import { drawEffects } from "../editor/drawEffects";
import ZoomTrack from "../editor/ZoomTrack";
import { cameraAt } from "../editor/zoomMath";

type Tab = "cut" | "effects" | "zoom" | "frame" | "export";
const TABS: { id: Tab; label: string }[] = [
  { id: "cut", label: "Cắt" },
  { id: "effects", label: "Hiệu ứng" },
  { id: "zoom", label: "Zoom" },
  { id: "frame", label: "Khung" },
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
  const [track, setTrack] = useState<[number, Camera][]>([]);
  const [layout, setLayout] = useState<Layout | null>(null);
  const [zoomSel, setZoomSel] = useState<number | null>(null);
  const [picking, setPicking] = useState(false);
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
      } else if (e.key === "Delete") {
        e.preventDefault();
        // A cut selection wins; a zoom segment is deleted only while it is shown and exists.
        if (selection) removeSelection();
        else if (zoomSel !== null && doc?.zoom.enabled && doc.zoom.segments[zoomSel]) {
          change({ ...doc, zoom: { ...doc.zoom, segments: doc.zoom.segments.filter((_, i) => i !== zoomSel) } });
          setZoomSel(null);
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [undoRedo, removeSelection, playhead, duration, frameMs, trimEnd, doc, zoomSel, change, selection]);

  // Moving the scale slider is one undo step, not one per tick.
  const scaleRun = useRef<{ index: number; at: number } | null>(null);
  const setZoomSegment = (index: number, patch: Partial<ZoomSegment>) => {
    const d = latest.current;
    if (!d || !d.zoom.segments[index]) return;
    const now = performance.now();
    const sliding = patch.scale !== undefined;
    const merge = sliding && scaleRun.current?.index === index && now - scaleRun.current.at < 1000;
    scaleRun.current = sliding ? { index, at: now } : null;
    const segments = d.zoom.segments.map((s, i) => (i === index ? { ...s, ...patch, manual: true } : s));
    change({ ...d, zoom: { ...d.zoom, segments } }, merge);
  };

  const regenerateZoom = async () => {
    const d = latest.current;
    if (!d || !session) return;
    try {
      const segments = await zoomAuto(props.path, d, frameOf(session));
      change({ ...d, zoom: { ...d.zoom, segments } });
      setZoomSel(null);
    } catch (e) {
      setMessage(errorMessage(e));
    }
  };

  const setZoomEnabled = async (enabled: boolean) => {
    const d = latest.current;
    if (!d || !session) return;
    let segments = d.zoom.segments;
    // First time on: start from the clicks.
    if (enabled && segments.length === 0 && session.effects) {
      try {
        segments = await zoomAuto(props.path, d, frameOf(session));
      } catch (e) {
        setMessage(errorMessage(e));
      }
    }
    if (!enabled) {
      setZoomSel(null);
      setPicking(false);
    }
    change({ ...d, zoom: { enabled, segments } });
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
          draw={
            session.effects
              ? (ctx, ms) => drawEffects(ctx, session.effects!, ms, { ...doc.effects, keys: false }, session.info.height, ctx.canvas.height / session.info.height)
              : undefined
          }
          drawFixed={
            session.effects
              ? (ctx, ms) => drawEffects(ctx, session.effects!, ms, { ...doc.effects, clicks: false }, session.info.height, ctx.canvas.height / session.info.height)
              : undefined
          }
          camera={doc.zoom.enabled && track.length > 0 ? (ms) => cameraAt(track, ms) : undefined}
          onPick={
            picking && zoomSel !== null
              ? (x, y) => {
                  setPicking(false);
                  setZoomSegment(zoomSel, { center: [x, y] });
                }
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
                  <span className="hint check-hint">Phóng to những chỗ bạn muốn người xem chú ý. Kéo trên rãnh Zoom (dưới dòng thời gian) để thêm một đoạn.</span>
                </span>
              </label>
              {doc.zoom.enabled && (
                <>
                  <button className="btn btn-small" disabled={!session.effects} onClick={() => void regenerateZoom()}>
                    Tự tạo lại theo click
                  </button>
                  {!session.effects && <p className="hint">Video này không có nhật ký thao tác: chỉ thêm zoom bằng tay được.</p>}
                  {zoomSel !== null && doc.zoom.segments[zoomSel] ? (
                    <ZoomControls
                      seg={doc.zoom.segments[zoomSel]}
                      picking={picking}
                      hasCursor={!!session.effects}
                      onScale={(scale) => setZoomSegment(zoomSel, { scale })}
                      onPick={() => setPicking(!picking)}
                      onFollow={() => setZoomSegment(zoomSel, { center: null })}
                      onDelete={() => {
                        change({ ...doc, zoom: { ...doc.zoom, segments: doc.zoom.segments.filter((_, i) => i !== zoomSel) } });
                        setZoomSel(null);
                      }}
                    />
                  ) : (
                    <p className="hint">Bấm vào một đoạn trên rãnh Zoom để chỉnh mức phóng và tâm.</p>
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
            playhead={playhead}
            onSelect={(i) => {
              setZoomSel(i);
              setPicking(false);
              if (i !== null) setTab("zoom");
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

/** Controls for the selected zoom segment. */
function ZoomControls(props: {
  seg: ZoomSegment;
  picking: boolean;
  hasCursor: boolean;
  onScale: (scale: number) => void;
  onPick: () => void;
  onFollow: () => void;
  onDelete: () => void;
}) {
  const { seg } = props;
  return (
    <div className="zoom-controls">
      <label className="zoom-scale">
        Mức phóng: <strong>{seg.scale.toFixed(1)}×</strong>
        <input type="range" min={1} max={4} step={0.1} value={seg.scale} onChange={(e) => props.onScale(Number(e.target.value))} />
      </label>
      {seg.scale > 2 && <p className="hint bar-warn">Phóng hơn 2× thì video có thể bị mờ.</p>}
      <p className="hint">
        Tâm: {seg.center ? "cố định" : props.hasCursor ? "đi theo con trỏ chuột" : "giữa khung hình"}
      </p>
      <div className="row">
        <button className={"btn btn-small" + (props.picking ? " btn-rec" : "")} onClick={props.onPick}>
          {props.picking ? "Bấm lên video…" : "Chọn tâm trên video"}
        </button>
        {seg.center && (
          <button className="btn btn-small" onClick={props.onFollow}>
            {props.hasCursor ? "Theo con trỏ" : "Về giữa"}
          </button>
        )}
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
