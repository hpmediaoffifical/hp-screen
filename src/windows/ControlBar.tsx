import { getCurrentWindow } from "@tauri-apps/api/window";
import { formatElapsed, recCancel, recStop, recTogglePause } from "../api";
import { useRecStatus } from "../useRecStatus";

/** Small always-on-top bar shown while recording. It is excluded from the recording. */
export default function ControlBar() {
  const { status } = useRecStatus();
  const { state } = status;
  const ignore = () => {};

  let label = "";
  if (state === "countdown") label = `Bắt đầu sau ${status.countdown ?? ""}`;
  else if (state === "recording") label = formatElapsed(status.elapsed_ms);
  else if (state === "paused") label = `Tạm dừng · ${formatElapsed(status.elapsed_ms)}`;
  else if (state === "finalizing") label = "Đang lưu…";

  return (
    <div
      className={"bar bar-" + state}
      title={status.warning ?? "Kéo để di chuyển"}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget || (e.target as HTMLElement).dataset.drag) {
          getCurrentWindow().startDragging().catch(ignore);
        }
      }}
    >
      <span className="bar-dot" data-drag="1" aria-hidden="true" />
      <span className="bar-time" data-drag="1" role="status">{label}</span>
      {status.warning && <span className="bar-warn" data-drag="1" aria-label={status.warning}>⚠</span>}
      <span className="bar-space" data-drag="1" />
      {state === "countdown" && (
        <button className="bar-btn" onClick={() => recCancel().catch(ignore)}>Hủy</button>
      )}
      {(state === "recording" || state === "paused") && (
        <>
          <button className="bar-btn" onClick={() => recTogglePause().catch(ignore)}>
            {state === "paused" ? "Tiếp tục" : "Tạm dừng"}
          </button>
          <button className="bar-btn bar-stop" onClick={() => recStop().catch(ignore)}>■ Dừng</button>
        </>
      )}
    </div>
  );
}
