import { useState } from "react";
import { errorMessage, formatElapsed, recCancel, recStart, recStop, recTogglePause, takeScreenshot } from "../api";
import { useRecStatus } from "../useRecStatus";

const STATE_LABEL = {
  idle: "Sẵn sàng",
  countdown: "Chuẩn bị quay",
  recording: "Đang quay",
  paused: "Đang tạm dừng",
  finalizing: "Đang lưu video…",
} as const;

// Temporary controls until the full record page lands (plan task 12).
export default function RecordPage() {
  const { status, error, clearError } = useRecStatus();
  const [note, setNote] = useState("");
  const { state } = status;

  async function run(action: () => Promise<unknown>, done?: (r: unknown) => string) {
    clearError();
    try {
      const result = await action();
      setNote(done ? done(result) : "");
    } catch (e) {
      setNote(errorMessage(e));
    }
  }

  return (
    <section className="page">
      <h1>Quay</h1>
      <p className="rec-line">
        <strong>{STATE_LABEL[state]}</strong>
        {state === "countdown" && status.countdown !== null && <span> · bắt đầu sau {status.countdown} giây</span>}
        {(state === "recording" || state === "paused") && <span> · {formatElapsed(status.elapsed_ms)}</span>}
      </p>
      <div className="row">
        {state === "idle" && (
          <button className="btn btn-rec" onClick={() => run(() => recStart({ kind: "monitor", index: 0 }))}>
            ● Bắt đầu quay màn hình
          </button>
        )}
        {state === "countdown" && (
          <button className="btn" onClick={() => run(recCancel)}>Hủy</button>
        )}
        {(state === "recording" || state === "paused") && (
          <>
            <button className="btn" onClick={() => run(recTogglePause)}>
              {state === "paused" ? "Tiếp tục" : "Tạm dừng"}
            </button>
            <button className="btn btn-rec" onClick={() => run(recStop)}>■ Dừng</button>
          </>
        )}
        <button
          className="btn"
          disabled={state === "finalizing"}
          onClick={() =>
            run(
              () => takeScreenshot({ kind: "monitor", index: 0 }),
              (path) => `Đã lưu ảnh và copy vào clipboard: ${path}`,
            )
          }
        >
          Chụp ảnh màn hình
        </button>
      </div>
      {status.file && <p className="status">Đang ghi vào: {status.file}</p>}
      {status.warning && <p className="status warn" role="status">{status.warning}</p>}
      {error && <p className="status err" role="alert">{error}</p>}
      <p className="status" role="status">{note}</p>
    </section>
  );
}
