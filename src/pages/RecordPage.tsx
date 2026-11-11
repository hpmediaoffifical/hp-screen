import { useEffect, useState } from "react";
import {
  audioStatus,
  errorMessage,
  formatElapsed,
  listMonitors,
  pickTarget,
  recCancel,
  recStart,
  recStop,
  recTogglePause,
  settingsGet,
  settingsSet,
  takeScreenshot,
  type AudioStatus,
  type MonitorInfo,
  type Rect,
  type Settings,
  type Target,
} from "../api";
import { useRecStatus } from "../useRecStatus";
import RecentItems from "../components/RecentItems";

type Mode = "monitor" | "window" | "region";

const STATE_LABEL = {
  idle: "Sẵn sàng",
  countdown: "Chuẩn bị quay",
  recording: "Đang quay",
  paused: "Đang tạm dừng",
  finalizing: "Đang lưu video…",
} as const;

/** The monitor whose area contains the rectangle's top-left corner. */
function monitorOf(rect: Rect, monitors: MonitorInfo[]): number | null {
  const m = monitors.find(
    (m) => rect.x >= m.rect.x && rect.x < m.rect.x + m.rect.w && rect.y >= m.rect.y && rect.y < m.rect.y + m.rect.h,
  );
  return m ? m.index : null;
}

export default function RecordPage() {
  const { status, error, clearError } = useRecStatus();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [audio, setAudio] = useState<AudioStatus | null>(null);
  const [mode, setMode] = useState<Mode>("monitor");
  const [monitorIndex, setMonitorIndex] = useState(0);
  const [windowTarget, setWindowTarget] = useState<Target | null>(null);
  const [regionTarget, setRegionTarget] = useState<Extract<Target, { kind: "region" }> | null>(null);
  const [note, setNote] = useState("");
  const { state } = status;
  const idle = state === "idle";

  useEffect(() => {
    settingsGet().then(setSettings).catch(() => {});
    audioStatus().then(setAudio).catch(() => {});
    listMonitors()
      .then((list) => {
        setMonitors(list);
        setMonitorIndex(list.find((m) => m.primary)?.index ?? 0);
      })
      .catch(() => {});
  }, []);

  const target: Target | null =
    mode === "monitor" ? { kind: "monitor", index: monitorIndex } : mode === "window" ? windowTarget : regionTarget;

  async function run(action: () => Promise<unknown>, done?: (r: unknown) => string) {
    clearError();
    try {
      const result = await action();
      setNote(done ? done(result) : "");
    } catch (e) {
      setNote(errorMessage(e));
    }
  }

  async function save(patch: Partial<Settings>) {
    if (!settings) return;
    try {
      setSettings(await settingsSet({ ...settings, ...patch }));
    } catch (e) {
      setNote(errorMessage(e));
    }
  }

  async function pick(kind: "window" | "region") {
    setMode(kind);
    try {
      const picked = await pickTarget(kind);
      if (!picked) return;
      if (picked.kind === "window") setWindowTarget(picked);
      if (picked.kind === "region") {
        setRegionTarget(picked);
        save({ last_region: picked.rect });
      }
    } catch (e) {
      setNote(errorMessage(e));
    }
  }

  function useLastRegion() {
    const rect = settings?.last_region;
    if (!rect) return;
    const monitor = monitorOf(rect, monitors);
    if (monitor === null) {
      setNote("Vùng gần nhất nằm trên màn hình không còn kết nối. Hãy chọn vùng mới.");
      return;
    }
    setMode("region");
    setRegionTarget({ kind: "region", monitor, rect });
  }

  const targetHint =
    mode === "window" && !windowTarget
      ? "Chưa chọn cửa sổ"
      : mode === "region" && !regionTarget
        ? "Chưa chọn vùng"
        : "";

  return (
    <section className="page record">
      <h1>Quay</h1>

      <h2 className="label">Chế độ quay</h2>
      <div className="modes" role="radiogroup" aria-label="Chế độ quay">
        <div className={"mode" + (mode === "monitor" ? " on" : "")}>
          <button role="radio" aria-checked={mode === "monitor"} className="mode-main" disabled={!idle} onClick={() => setMode("monitor")}>
            <span className="mode-title">Toàn màn hình</span>
            <span className="mode-sub">
              {monitors.find((m) => m.index === monitorIndex)
                ? `${monitors.find((m) => m.index === monitorIndex)!.rect.w} × ${monitors.find((m) => m.index === monitorIndex)!.rect.h}`
                : "Màn hình chính"}
            </span>
          </button>
          {monitors.length > 1 && (
            <select
              aria-label="Chọn màn hình"
              value={monitorIndex}
              disabled={!idle}
              onChange={(e) => {
                setMonitorIndex(Number(e.target.value));
                setMode("monitor");
              }}
            >
              {monitors.map((m) => (
                <option key={m.index} value={m.index}>
                  {m.name}
                  {m.primary ? " (chính)" : ""}
                </option>
              ))}
            </select>
          )}
        </div>

        <div className={"mode" + (mode === "window" ? " on" : "")}>
          <button role="radio" aria-checked={mode === "window"} className="mode-main" disabled={!idle} onClick={() => (windowTarget ? setMode("window") : pick("window"))}>
            <span className="mode-title">Cửa sổ</span>
            <span className="mode-sub">{windowTarget ? "Đã chọn một cửa sổ" : "Chỉ quay một cửa sổ"}</span>
          </button>
          <button className="btn btn-small" disabled={!idle} onClick={() => pick("window")}>
            {windowTarget ? "Chọn lại" : "Chọn cửa sổ"}
          </button>
        </div>

        <div className={"mode" + (mode === "region" ? " on" : "")}>
          <button role="radio" aria-checked={mode === "region"} className="mode-main" disabled={!idle} onClick={() => (regionTarget ? setMode("region") : pick("region"))}>
            <span className="mode-title">Vùng chọn</span>
            <span className="mode-sub">{regionTarget ? `${regionTarget.rect.w} × ${regionTarget.rect.h}` : "Kéo chọn một vùng"}</span>
          </button>
          <div className="row">
            <button className="btn btn-small" disabled={!idle} onClick={() => pick("region")}>
              {regionTarget ? "Chọn lại" : "Chọn vùng"}
            </button>
            {settings?.last_region && !regionTarget && (
              <button className="btn btn-small" disabled={!idle} onClick={useLastRegion}>
                Vùng gần nhất
              </button>
            )}
          </div>
        </div>
      </div>
      {mode === "window" && (
        <p className="hint">Chế độ này chỉ ghi nội dung của cửa sổ đã chọn, kể cả khi bị cửa sổ khác che. Menu chuột phải và hộp thoại bật ra có thể không được ghi.</p>
      )}

      <h2 className="label">Nguồn âm thanh</h2>
      <div className="row">
        <Toggle
          label="Mic"
          on={!!settings?.mic}
          unavailable={audio !== null && !audio.mic_available}
          disabled={!idle || !settings}
          onChange={(mic) => save({ mic })}
        />
        <Toggle
          label="Âm thanh máy"
          on={!!settings?.system_audio}
          unavailable={audio !== null && !audio.system_available}
          disabled={!idle || !settings}
          onChange={(system_audio) => save({ system_audio })}
        />
        <span className="chip">MP4 · {settings?.fps ?? 30} fps</span>
      </div>

      <div className="row actions">
        {idle && (
          <button className="btn btn-rec btn-big" disabled={!target} onClick={() => target && run(() => recStart(target))}>
            ● Bắt đầu quay
            {settings && <kbd>{settings.hotkey_toggle}</kbd>}
          </button>
        )}
        {state === "countdown" && (
          <button className="btn btn-big" onClick={() => run(recCancel)}>
            Hủy · bắt đầu sau {status.countdown ?? ""} giây
          </button>
        )}
        {(state === "recording" || state === "paused") && (
          <>
            <button className="btn btn-rec btn-big" onClick={() => run(recStop)}>
              ■ Dừng · {formatElapsed(status.elapsed_ms)}
            </button>
            <button className="btn btn-big" onClick={() => run(recTogglePause)}>
              {state === "paused" ? "Tiếp tục" : "Tạm dừng"}
            </button>
          </>
        )}
        {state === "finalizing" && <span className="status">{STATE_LABEL.finalizing}</span>}
        <button
          className="btn btn-big"
          disabled={!target || state === "finalizing"}
          onClick={() => target && run(() => takeScreenshot(target), (path) => `Đã lưu ảnh và copy vào clipboard: ${path}`)}
        >
          Chụp ảnh
          {settings && <kbd>{settings.hotkey_screenshot}</kbd>}
        </button>
        {targetHint && idle && <span className="status warn">{targetHint}</span>}
      </div>

      {status.warning && <p className="status warn" role="status">{status.warning}</p>}
      {error && <p className="status err" role="alert">{error}</p>}
      {note && <p className="status" role="status">{note}</p>}

      <h2 className="label">Vừa quay</h2>
      <RecentItems />
    </section>
  );
}

function Toggle(props: { label: string; on: boolean; unavailable: boolean; disabled: boolean; onChange: (on: boolean) => void }) {
  const { label, on, unavailable, disabled, onChange } = props;
  return (
    <button
      role="switch"
      aria-checked={on}
      className={"chip chip-toggle" + (on && !unavailable ? " on" : "")}
      disabled={disabled}
      onClick={() => onChange(!on)}
    >
      {label}: {unavailable ? "không có thiết bị" : on ? "bật" : "tắt"}
    </button>
  );
}
