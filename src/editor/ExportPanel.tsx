import { useEffect, useState } from "react";
import { errorMessage, formatElapsed, libraryOpen, libraryReveal } from "../api";
import { exportCancel, exportStart, onExportDone, onExportError, onExportProgress, type EditDoc, type GifOptions } from "../editApi";

type Job = { kind: "idle" } | { kind: "running"; done: number; total: number } | { kind: "done"; path: string } | { kind: "error"; message: string };

const nameOf = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** The Export tab: start, follow and cancel an export, then open the result. */
export default function ExportPanel(props: {
  hidden: boolean;
  path: string;
  doc: EditDoc;
  canExport: boolean;
  outputMs: number;
  beforeExport: () => Promise<void>;
}) {
  const [job, setJob] = useState<Job>({ kind: "idle" });
  const [format, setFormat] = useState<"mp4" | "gif">("mp4");
  const [gif, setGif] = useState<GifOptions>({ fps: 15, width: 720 });
  const { path } = props;

  useEffect(() => {
    const subs = [
      onExportProgress((source, done, total) => source === path && setJob({ kind: "running", done, total })),
      onExportDone((source, out) => source === path && setJob({ kind: "done", path: out })),
      onExportError((source, message) => source === path && setJob({ kind: "error", message })),
    ];
    return () => subs.forEach((s) => s.then((u) => u()));
  }, [path]);

  async function start() {
    try {
      await props.beforeExport();
      setJob({ kind: "running", done: 0, total: 1 });
      await exportStart(props.path, props.doc, format === "gif" ? gif : null);
    } catch (e) {
      setJob({ kind: "error", message: errorMessage(e) });
    }
  }

  const running = job.kind === "running";
  return (
    <div className="panel-body" hidden={props.hidden}>
      <p className="hint">
        Dài {formatElapsed(props.outputMs)}, lưu cạnh video gốc với hậu tố <code>_edit</code>. Video gốc giữ nguyên.
      </p>
      <div className="row" role="radiogroup" aria-label="Định dạng">
        {(["mp4", "gif"] as const).map((f) => (
          <button key={f} className={"chip chip-toggle" + (format === f ? " on" : "")} aria-pressed={format === f} onClick={() => setFormat(f)}>
            {f.toUpperCase()}
          </button>
        ))}
      </div>
      {format === "gif" && (
        <>
          <div className="row" role="radiogroup" aria-label="Số khung mỗi giây">
            {[10, 15].map((fps) => (
              <button key={fps} className={"chip chip-toggle" + (gif.fps === fps ? " on" : "")} aria-pressed={gif.fps === fps} onClick={() => setGif({ ...gif, fps })}>
                {fps} fps
              </button>
            ))}
          </div>
          <div className="row" role="radiogroup" aria-label="Chiều rộng">
            {[480, 720, 960].map((width) => (
              <button key={width} className={"chip chip-toggle" + (gif.width === width ? " on" : "")} aria-pressed={gif.width === width} onClick={() => setGif({ ...gif, width })}>
                {width} px
              </button>
            ))}
          </div>
          <p className="hint">GIF không có tiếng.</p>
          {props.outputMs > 60_000 && <p className="hint bar-warn">GIF dài hơn 1 phút sẽ rất nặng. Nên cắt ngắn bớt.</p>}
        </>
      )}
      <div className="row">
        <button className="btn btn-rec" disabled={!props.canExport || running} onClick={start}>
          Xuất {format.toUpperCase()}
        </button>
        {running && (
          <button className="btn" onClick={() => exportCancel().catch(() => {})}>
            Hủy
          </button>
        )}
      </div>
      {!props.canExport && <p className="hint">Phần giữ lại quá ngắn để xuất (cần ít nhất 0,5 giây).</p>}
      {running && (
        <div className="progress" role="progressbar" aria-valuemin={0} aria-valuemax={job.total} aria-valuenow={job.done}>
          <div className="progress-fill" style={{ width: `${(job.done / Math.max(1, job.total)) * 100}%` }} />
        </div>
      )}
      {job.kind === "done" && (
        <div className="export-done" role="status">
          <span>Đã lưu {nameOf(job.path)}</span>
          <div className="row">
            <button className="btn btn-small" onClick={() => libraryOpen(job.path).catch(() => {})}>Phát</button>
            <button className="btn btn-small" onClick={() => libraryReveal(job.path).catch(() => {})}>Thư mục</button>
          </div>
        </div>
      )}
      {job.kind === "error" && <p className="status bar-warn" role="alert">{job.message}</p>}
    </div>
  );
}
