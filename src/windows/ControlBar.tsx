import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { useEffect, useLayoutEffect, useRef, useState, type MouseEvent, type ReactNode } from "react";
import {
  audioStatus,
  compactHide,
  errorMessage,
  formatElapsed,
  getTarget,
  listMonitors,
  onSettingsChanged,
  onTargetChanged,
  openMain,
  pickTarget,
  recCancel,
  recStart,
  recStop,
  recTogglePause,
  settingsGet,
  settingsSet,
  setTarget,
  snipStart,
  takeScreenshot,
  type AudioStatus,
  type Settings,
  type Target,
} from "../api";
import { useRecStatus } from "../useRecStatus";

/**
 * The compact bar: one always-on-top row with the everyday controls, so recording and
 * screenshots work without the main window. Drag it by the grip; fold it into a small pill.
 * It is excluded from every capture.
 */
export default function ControlBar() {
  const { status, error, clearError } = useRecStatus();
  const [collapsed, setCollapsed] = useState(false);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [audio, setAudio] = useState<AudioStatus | null>(null);
  const [target, setLocalTarget] = useState<Target | null>(null);
  const [note, setNote] = useState("");
  const rowRef = useRef<HTMLDivElement>(null);
  const { state } = status;
  const active = state !== "idle";

  useEffect(() => {
    settingsGet().then(setSettings).catch(() => {});
    audioStatus().then(setAudio).catch(() => {});
    getTarget().then(setLocalTarget).catch(() => {});
    const unlisten = [onSettingsChanged(setSettings), onTargetChanged(setLocalTarget)];
    return () => unlisten.forEach((p) => p.then((off) => off()));
  }, []);

  // The window is exactly as large as the row it shows.
  useLayoutEffect(() => {
    const row = rowRef.current;
    if (!row) return;
    const fit = () => {
      const { width, height } = row.getBoundingClientRect();
      getCurrentWindow()
        .setSize(new LogicalSize(Math.ceil(width), Math.ceil(height)))
        .catch(() => {});
    };
    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(row);
    return () => observer.disconnect();
  }, []);

  // Messages disappear on their own.
  useEffect(() => {
    if (!note && !error) return;
    const timer = setTimeout(() => {
      setNote("");
      clearError();
    }, 4000);
    return () => clearTimeout(timer);
  }, [note, error, clearError]);

  async function run(action: () => Promise<unknown>) {
    clearError();
    setNote("");
    try {
      await action();
    } catch (e) {
      setNote(errorMessage(e));
    }
  }

  async function chooseMonitor() {
    const monitors = await listMonitors();
    const index = monitors.find((m) => m.primary)?.index ?? 0;
    await setTarget({ kind: "monitor", index });
  }

  async function choose(kind: "window" | "region") {
    const picked = await pickTarget(kind);
    if (picked) await setTarget(picked);
  }

  async function startRecording() {
    const current = target ?? (await getTarget());
    await recStart(current);
  }

  const save = (patch: Partial<Settings>) => run(async () => settings && setSettings(await settingsSet({ ...settings, ...patch })));

  const drag = (e: MouseEvent) => {
    if (e.button === 0) getCurrentWindow().startDragging().catch(() => {});
  };

  const message = error || note || (active ? status.warning : null);
  const kind = target?.kind;

  return (
    <div ref={rowRef} className={"bar bar-" + state + (collapsed ? " bar-collapsed" : "")}>
      <span className="bar-grip" onMouseDown={drag} title="Kéo để di chuyển" aria-hidden="true">
        ⋮⋮
      </span>

      {active && (
        <span className="bar-status" onMouseDown={drag} role="timer" aria-live="off">
          <span className="bar-dot" aria-hidden="true" />
          {state === "countdown" && `Bắt đầu sau ${status.countdown ?? ""}`}
          {state === "recording" && formatElapsed(status.elapsed_ms)}
          {state === "paused" && `Tạm dừng · ${formatElapsed(status.elapsed_ms)}`}
          {state === "finalizing" && "Đang lưu…"}
        </span>
      )}
      {!active && collapsed && (
        <span className="bar-brand" onMouseDown={drag}>
          HP
        </span>
      )}

      {!collapsed && !active && (
        <>
          <Group label="Chế độ quay">
            <BarButton on={kind === "monitor"} title="Quay toàn màn hình" onClick={() => run(chooseMonitor)}>
              ▣ Toàn màn
            </BarButton>
            <BarButton on={kind === "window"} title="Chọn một cửa sổ để quay" onClick={() => run(() => choose("window"))}>
              ▢ Cửa sổ
            </BarButton>
            <BarButton on={kind === "region"} title="Kéo chọn vùng để quay" onClick={() => run(() => choose("region"))}>
              ⬚ Vùng
            </BarButton>
          </Group>
          <button className="bar-btn bar-rec" title={`Bắt đầu quay (${settings?.hotkey_toggle ?? ""})`} onClick={() => run(startRecording)}>
            ● Quay
          </button>
          <span className="bar-sep" />
          <BarButton title={`Chụp ảnh và vẽ ghi chú (${settings?.hotkey_screenshot ?? ""})`} onClick={() => run(snipStart)}>
            ✎ Chụp &amp; vẽ
          </BarButton>
          <BarButton
            title={`Chụp nhanh theo chế độ đang chọn (${settings?.hotkey_quickshot ?? ""})`}
            onClick={() => run(async () => target && setNote(`Đã lưu ${(await takeScreenshot(target)).split(/[\\/]/).pop()}`))}
          >
            ⧉ Chụp nhanh
          </BarButton>
          <span className="bar-sep" />
          <AudioToggle
            label="Mic"
            icon="🎤"
            on={!!settings?.mic}
            missing={audio !== null && !audio.mic_available}
            onClick={() => save({ mic: !settings?.mic })}
          />
          <AudioToggle
            label="Âm thanh máy"
            icon="🔊"
            on={!!settings?.system_audio}
            missing={audio !== null && !audio.system_available}
            onClick={() => save({ system_audio: !settings?.system_audio })}
          />
        </>
      )}

      {!collapsed && state === "countdown" && (
        <BarButton title="Hủy" onClick={() => run(recCancel)}>
          Hủy
        </BarButton>
      )}
      {!collapsed && (state === "recording" || state === "paused") && (
        <>
          <BarButton title={`Tạm dừng / tiếp tục (${settings?.hotkey_pause ?? ""})`} onClick={() => run(recTogglePause)}>
            {state === "paused" ? "▶ Tiếp tục" : "❚❚ Tạm dừng"}
          </BarButton>
          <button className="bar-btn bar-rec" title={`Dừng và lưu (${settings?.hotkey_toggle ?? ""})`} onClick={() => run(recStop)}>
            ■ Dừng
          </button>
        </>
      )}
      {collapsed && (state === "recording" || state === "paused") && (
        <button className="bar-btn bar-icon bar-rec" title="Dừng và lưu" aria-label="Dừng và lưu" onClick={() => run(recStop)}>
          ■
        </button>
      )}

      {message && !collapsed && (
        <span className="bar-msg" role="status" title={message}>
          {message}
        </span>
      )}

      <span className="bar-sep" />
      {!collapsed && (
        <BarButton icon title="Mở HP Screen đầy đủ" onClick={() => run(openMain)}>
          ⤢
        </BarButton>
      )}
      <BarButton icon title={collapsed ? "Mở rộng thanh" : "Thu nhỏ thanh"} onClick={() => setCollapsed(!collapsed)}>
        {collapsed ? "▸" : "◂"}
      </BarButton>
      {!collapsed && !active && (
        <BarButton icon title="Ẩn thanh (HP Screen vẫn chạy ở khay hệ thống)" onClick={() => run(compactHide)}>
          ✕
        </BarButton>
      )}
    </div>
  );
}

function Group(props: { label: string; children: ReactNode }) {
  return (
    <span className="bar-group" role="group" aria-label={props.label}>
      {props.children}
    </span>
  );
}

function BarButton(props: { title: string; on?: boolean; icon?: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      className={"bar-btn" + (props.on ? " on" : "") + (props.icon ? " bar-icon" : "")}
      title={props.title}
      aria-label={props.title}
      aria-pressed={props.on}
      onClick={props.onClick}
    >
      {props.children}
    </button>
  );
}

function AudioToggle(props: { label: string; icon: string; on: boolean; missing: boolean; onClick: () => void }) {
  const state = props.missing ? "không có thiết bị" : props.on ? "bật" : "tắt";
  return (
    <button
      className={"bar-btn bar-icon bar-audio" + (props.on && !props.missing ? " on" : " off")}
      role="switch"
      aria-checked={props.on}
      aria-label={`${props.label}: ${state}`}
      title={`${props.label}: ${state}`}
      onClick={props.onClick}
    >
      {props.icon}
    </button>
  );
}
