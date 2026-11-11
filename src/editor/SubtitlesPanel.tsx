import { useEffect, useRef, useState } from "react";
import { errorMessage, libraryReveal } from "../api";
import {
  asrCancel,
  asrInstall,
  asrStatus,
  exportSrt,
  formatPrecise,
  onAsrDone,
  onAsrError,
  onAsrProgress,
  onSubsDone,
  onSubsError,
  onSubsProgress,
  subtitlesGenerate,
  type AsrStatus,
  type Cue,
  type EditDoc,
  type SubtitleDoc,
} from "../editApi";
import { deleteCue, mergeWithNext, setCueText, setCueTime, splitCue } from "./subsMath";

type Job =
  | { kind: "idle" }
  | { kind: "install"; phase: "cli" | "model"; done: number; total: number }
  | { kind: "generate"; phase: "audio" | "recognise"; percent: number }
  | { kind: "error"; message: string };

const nameOf = (path: string) => path.split(/[\\/]/).pop() ?? path;
const mb = (bytes: number) => Math.round(bytes / 1_000_000);

/**
 * The "Phụ đề" tab: make Vietnamese subtitles from the video's sound on this computer (the
 * recogniser is downloaded once, only after the user agrees), fix them line by line, burn them
 * into the export or save an SRT. Always mounted, so a running job keeps its progress.
 */
export default function SubtitlesPanel(props: {
  hidden: boolean;
  path: string;
  doc: EditDoc;
  durationMs: number;
  hasAudio: boolean;
  playhead: number;
  onChange: (subs: SubtitleDoc, merge: boolean) => void;
  onSeek: (ms: number) => void;
  beforeExport: () => Promise<void>;
}) {
  const { path } = props;
  const subs = props.doc.subtitles;
  const [status, setStatus] = useState<AsrStatus | null>(null);
  const [model, setModel] = useState<string | null>(null);
  const [consent, setConsent] = useState(false);
  const [job, setJob] = useState<Job>({ kind: "idle" });
  const [srt, setSrt] = useState<string | null>(null);
  // After an install finishes, recognition starts on its own.
  const thenGenerate = useRef<string | null>(null);
  // A cancelled job's error event is not shown.
  const cancelled = useRef(false);

  const refresh = () =>
    asrStatus()
      .then((s) => {
        setStatus(s);
        setModel((m) => m ?? (s.models.find((x) => x.installed) ?? s.models[0])?.id ?? null);
        return s;
      })
      .catch((e) => {
        setJob({ kind: "error", message: errorMessage(e) });
        return null;
      });

  useEffect(() => {
    void refresh();
  }, []);

  const generate = async (id: string) => {
    cancelled.current = false;
    try {
      await props.beforeExport();
      setSrt(null);
      setJob({ kind: "generate", phase: "audio", percent: 0 });
      await subtitlesGenerate(path, id);
    } catch (e) {
      setJob({ kind: "error", message: errorMessage(e) });
    }
  };

  // The latest props for event handlers that live as long as the panel.
  const live = useRef(props);
  live.current = props;
  useEffect(() => {
    const unlisten = [
      onAsrProgress((phase, done, total) => setJob({ kind: "install", phase, done, total })),
      onAsrDone(() => {
        const id = thenGenerate.current;
        thenGenerate.current = null;
        void refresh().then(() => (id ? generate(id) : setJob({ kind: "idle" })));
      }),
      onAsrError((message) => {
        thenGenerate.current = null;
        setJob(cancelled.current ? { kind: "idle" } : { kind: "error", message });
      }),
      onSubsProgress((source, phase, percent) => source === path && setJob({ kind: "generate", phase, percent })),
      onSubsDone((source, cues) => {
        if (source !== path) return;
        setJob(cues.length === 0 ? { kind: "error", message: "Không nghe ra lời nói nào trong video." } : { kind: "idle" });
        if (cues.length > 0) live.current.onChange({ ...live.current.doc.subtitles, cues }, false);
      }),
      onSubsError((source, message) => source === path && setJob(cancelled.current ? { kind: "idle" } : { kind: "error", message })),
    ];
    return () => unlisten.forEach((u) => u.then((f) => f()));
  }, [path]);

  const chosen = status?.models.find((m) => m.id === model) ?? null;
  const ready = !!status?.cli && !!chosen?.installed;
  const busy = job.kind === "install" || job.kind === "generate";

  const start = () => {
    if (!chosen) return;
    if (ready) void generate(chosen.id);
    else setConsent(true);
  };

  const agree = async () => {
    if (!chosen) return;
    setConsent(false);
    cancelled.current = false;
    thenGenerate.current = chosen.id;
    try {
      setJob({ kind: "install", phase: status?.cli ? "model" : "cli", done: 0, total: 1 });
      await asrInstall(chosen.id);
    } catch (e) {
      thenGenerate.current = null;
      setJob({ kind: "error", message: errorMessage(e) });
    }
  };

  const saveSrt = async () => {
    try {
      await props.beforeExport();
      setSrt(await exportSrt(path, live.current.doc, props.durationMs));
    } catch (e) {
      setJob({ kind: "error", message: errorMessage(e) });
    }
  };

  // Typing in one line is one undo step, not one per key.
  const typing = useRef<{ index: number; at: number } | null>(null);
  const setCues = (cues: Cue[], typedIn: number | null = null) => {
    const now = performance.now();
    const merge = typedIn !== null && typing.current?.index === typedIn && now - typing.current.at < 1500;
    typing.current = typedIn !== null ? { index: typedIn, at: now } : null;
    props.onChange({ ...live.current.doc.subtitles, cues }, merge);
  };

  const addAtPlayhead = () => {
    const at = Math.round(props.playhead);
    const cue = { start_ms: at, end_ms: Math.min(at + 2000, Math.max(at + 100, props.durationMs)), text: "" };
    const cues = [...subs.cues, cue].sort((a, b) => a.start_ms - b.start_ms);
    setCues(cues);
  };

  const ph = props.playhead;
  return (
    <div className="panel-body" hidden={props.hidden}>
      <p className="hint">Tạo phụ đề tiếng Việt từ tiếng nói trong video, ngay trên máy này: âm thanh không rời khỏi máy.</p>
      {status && status.models.length > 1 && (
        <div className="row" role="radiogroup" aria-label="Bộ nhận giọng">
          {status.models.map((m) => (
            <button
              key={m.id}
              className={"chip chip-toggle" + (model === m.id ? " on" : "")}
              aria-pressed={model === m.id}
              disabled={busy}
              title={`${m.size_mb} MB${m.installed ? ", đã tải" : ""}`}
              onClick={() => setModel(m.id)}
            >
              {m.name}
              {m.installed ? " ✓" : ""}
            </button>
          ))}
        </div>
      )}
      {!props.hasAudio && <p className="hint bar-warn">Video này không có tiếng nên không tự tạo phụ đề được. Bạn vẫn thêm dòng bằng tay được.</p>}
      <div className="row">
        <button className="btn btn-rec" disabled={!chosen || busy || !props.hasAudio} onClick={start}>
          {subs.cues.length > 0 ? "Tạo lại phụ đề" : "Tạo phụ đề"}
        </button>
        {busy && (
          <button
            className="btn"
            onClick={() => {
              cancelled.current = true;
              thenGenerate.current = null;
              asrCancel().catch(() => {});
            }}
          >
            Hủy
          </button>
        )}
      </div>
      {subs.cues.length > 0 && !busy && <p className="hint">Tạo lại sẽ thay các dòng hiện có (bấm Ctrl+Z để lấy lại).</p>}
      {job.kind === "install" && (
        <>
          <p className="hint">
            {job.phase === "cli" ? "Đang tải chương trình nhận giọng" : "Đang tải model nhận giọng"}: {mb(job.done)} / {mb(job.total)} MB
          </p>
          <div className="progress" role="progressbar" aria-valuemin={0} aria-valuemax={job.total} aria-valuenow={job.done}>
            <div className="progress-fill" style={{ width: `${(job.done / Math.max(1, job.total)) * 100}%` }} />
          </div>
        </>
      )}
      {job.kind === "generate" && (
        <>
          <p className="hint">{job.phase === "audio" ? "Đang lấy âm thanh…" : `Đang nhận giọng: ${job.percent}%`}</p>
          <div className="progress" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={job.percent}>
            <div className="progress-fill" style={{ width: `${job.percent}%` }} />
          </div>
        </>
      )}
      {job.kind === "error" && <p className="status bar-warn" role="alert">{job.message}</p>}

      <label className="check">
        <input type="checkbox" checked={subs.burn} disabled={subs.cues.length === 0} onChange={(e) => props.onChange({ ...subs, burn: e.target.checked }, false)} />
        <span>
          Ghi cứng phụ đề vào video
          <span className="hint check-hint">Chữ hiện luôn trong video xuất ra. Bỏ chọn nếu bạn chỉ cần file SRT (ví dụ để đưa vào CapCut).</span>
        </span>
      </label>
      <div className="row">
        <button className="btn btn-small" disabled={subs.cues.length === 0} onClick={() => void saveSrt()}>Xuất SRT</button>
        <button className="btn btn-small" onClick={addAtPlayhead}>Thêm dòng tại đây</button>
      </div>
      {srt && (
        <div className="export-done" role="status">
          <span>Đã lưu {nameOf(srt)}</span>
          <button className="btn btn-small" onClick={() => libraryReveal(srt).catch(() => {})}>Thư mục</button>
        </div>
      )}

      {subs.cues.length > 0 && (
        <ol className="cue-list" aria-label="Các dòng phụ đề">
          {subs.cues.map((c, i) => {
            const on = c.start_ms <= ph && ph < c.end_ms;
            return (
              <li key={i} className={"cue" + (on ? " on" : "")}>
                <div className="cue-head">
                  <button className="cue-time" title="Tua tới dòng này" onClick={() => props.onSeek(c.start_ms)}>
                    {formatPrecise(c.start_ms)} – {formatPrecise(c.end_ms)}
                  </button>
                  <button className="btn btn-tiny" title="Đặt đầu dòng tại vị trí đang phát" disabled={ph >= c.end_ms} onClick={() => setCues(setCueTime(subs.cues, i, ph, c.end_ms))}>Đầu</button>
                  <button className="btn btn-tiny" title="Đặt cuối dòng tại vị trí đang phát" disabled={ph <= c.start_ms} onClick={() => setCues(setCueTime(subs.cues, i, c.start_ms, ph))}>Cuối</button>
                </div>
                <textarea
                  className="cue-text"
                  rows={2}
                  value={c.text}
                  aria-label={`Dòng ${i + 1}`}
                  onFocus={() => props.onSeek(c.start_ms)}
                  onChange={(e) => setCues(setCueText(subs.cues, i, e.target.value), i)}
                />
                <div className="row">
                  <button className="btn btn-tiny" disabled={i === subs.cues.length - 1} onClick={() => setCues(mergeWithNext(subs.cues, i))}>Gộp với dòng sau</button>
                  <button className="btn btn-tiny" disabled={!on} title="Tách dòng tại vị trí đang phát" onClick={() => setCues(splitCue(subs.cues, i, ph))}>Tách tại đây</button>
                  <button className="btn btn-tiny btn-danger" onClick={() => setCues(deleteCue(subs.cues, i))}>Xóa</button>
                </div>
              </li>
            );
          })}
        </ol>
      )}

      {consent && chosen && status && (
        <Consent status={status} model={chosen} onCancel={() => setConsent(false)} onAgree={() => void agree()} />
      )}
    </div>
  );
}

/** Asks before anything is downloaded: what, how big, from where, to where. */
function Consent(props: { status: AsrStatus; model: AsrStatus["models"][number]; onCancel: () => void; onAgree: () => void }) {
  const cancel = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    cancel.current?.focus();
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && props.onCancel();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [props]);
  const { status, model } = props;
  const total = (status.cli ? 0 : status.cli_size_mb) + (model.installed ? 0 : model.size_mb);
  return (
    <div className="modal-back" onClick={props.onCancel}>
      <div className="modal" role="alertdialog" aria-modal="true" aria-labelledby="asr-title" onClick={(e) => e.stopPropagation()}>
        <h2 id="asr-title">Tải bộ nhận giọng (khoảng {total} MB)?</h2>
        <p className="hint">Chỉ tải một lần. Sau đó tạo phụ đề không cần mạng, âm thanh không rời khỏi máy.</p>
        <ul className="hint consent-list">
          {!status.cli && <li>Chương trình whisper.cpp ({status.cli_size_mb} MB) từ github.com/ggml-org/whisper.cpp</li>}
          {!model.installed && <li>Model {model.name} ({model.size_mb} MB) từ huggingface.co/ggerganov/whisper.cpp</li>}
        </ul>
        <p className="hint">Lưu vào <code>{status.folder}</code>. Mỗi file được kiểm tra mã SHA-256 trước khi dùng.</p>
        <div className="row modal-actions">
          <button ref={cancel} className="btn" onClick={props.onCancel}>Để sau</button>
          <button className="btn btn-rec" onClick={props.onAgree}>Tải và tạo phụ đề</button>
        </div>
      </div>
    </div>
  );
}
