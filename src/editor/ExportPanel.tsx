import { useEffect, useState } from "react";
import { errorMessage, formatElapsed, libraryOpen, libraryReveal } from "../api";
import { exportCancel, exportStart, onExportDone, onExportError, onExportProgress, type EditDoc } from "../editApi";

type Job = { kind: "idle" } | { kind: "running"; done: number; total: number } | { kind: "done"; path: string } | { kind: "error"; message: string };

const nameOf = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** The Export tab: start, follow and cancel an export, then open the result. */
export default function ExportPanel(props: { path: string; doc: EditDoc; canExport: boolean; outputMs: number; beforeExport: () => Promise<void> }) {
  const [job, setJob] = useState<Job>({ kind: "idle" });

  useEffect(() => {
    const subs = [
      onExportProgress((done, total) => setJob({ kind: "running", done, total })),
      onExportDone((path) => setJob({ kind: "done", path })),
      onExportError((message) => setJob({ kind: "error", message })),
    ];
    return () => subs.forEach((s) => s.then((u) => u()));
  }, []);

  async function start() {
    try {
      await props.beforeExport();
      setJob({ kind: "running", done: 0, total: 1 });
      await exportStart(props.path, props.doc);
    } catch (e) {
      setJob({ kind: "error", message: errorMessage(e) });
    }
  }

  const running = job.kind === "running";
  return (
    <div className="panel-body">
      <p className="hint">
        Video mới dài {formatElapsed(props.outputMs)}, lưu cạnh video gốc với hậu tố <code>_edit</code>. Video gốc giữ nguyên.
      </p>
      <div className="row">
        <button className="btn btn-rec" disabled={!props.canExport || running} onClick={start}>
          Xuất MP4
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
