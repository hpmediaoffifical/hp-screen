import { useEffect, useState } from "react";
import { errorMessage, hotkeyStatus, onHotkeysChanged, settingsGet, settingsSet, type HotkeyStatus } from "../api";

/** Settings field and name of each hotkey action. */
export const HOTKEY_FIELDS = {
  toggle: { field: "hotkey_toggle", label: "Bật / tắt quay" },
  pause: { field: "hotkey_pause", label: "Tạm dừng / tiếp tục" },
  screenshot: { field: "hotkey_screenshot", label: "Chụp ảnh và vẽ ghi chú" },
  quickshot: { field: "hotkey_quickshot", label: "Chụp nhanh (không vẽ)" },
  draw: { field: "hotkey_draw", label: "Vẽ lên màn hình khi quay" },
} as const;
export type HotkeyField = (typeof HOTKEY_FIELDS)[keyof typeof HOTKEY_FIELDS]["field"];

/** Hotkeys another app holds right now; kept up to date. */
export function useTakenHotkeys(): HotkeyStatus[] {
  const [taken, setTaken] = useState<HotkeyStatus[]>([]);
  useEffect(() => {
    const update = (all: HotkeyStatus[]) => setTaken(all.filter((h) => !h.ok));
    hotkeyStatus().then(update).catch(() => {});
    const unlisten = onHotkeysChanged(update);
    return () => {
      unlisten.then((off) => off());
    };
  }, []);
  return taken;
}

/** Switches one action to another hotkey; resolves to an error message, or "" when it worked. */
export async function switchHotkey(action: HotkeyStatus["action"], hotkey: string): Promise<string> {
  try {
    const settings = await settingsGet();
    await settingsSet({ ...settings, [HOTKEYS_FIELD(action)]: hotkey });
    return "";
  } catch (e) {
    return errorMessage(e);
  }
}
const HOTKEYS_FIELD = (action: HotkeyStatus["action"]): HotkeyField => HOTKEY_FIELDS[action].field;

/**
 * Strip at the top of the main window when another app (Zalo, for one) holds one of HP
 * Screen's hotkeys, so the key would silently do nothing. Offers a free combination in one
 * click, or the hotkey settings.
 */
export default function HotkeyWarning({ onOpenSettings }: { onOpenSettings: (field: HotkeyField) => void }) {
  const taken = useTakenHotkeys();
  const [dismissed, setDismissed] = useState("");
  const [error, setError] = useState("");
  const key = taken.map((t) => t.hotkey).join(",");
  if (taken.length === 0 || dismissed === key) return null;

  return (
    <div className="hotkey-warning" role="alert">
      <span className="hotkey-warning-icon" aria-hidden="true">⚠</span>
      <div className="hotkey-warning-body">
        {taken.map((t) => (
          <div key={t.action} className="hotkey-warning-row">
            <span>
              Phím <b>{t.hotkey}</b> dùng để {HOTKEY_FIELDS[t.action].label.toLowerCase()} đang bị{" "}
              {t.message?.includes("Zalo") ? "Zalo" : "một ứng dụng khác"} giữ, nên bấm sẽ không có tác dụng.
            </span>
            {t.suggestion && (
              <button
                className="btn btn-small btn-primary"
                onClick={async () => setError(await switchHotkey(t.action, t.suggestion!))}
              >
                Đổi sang {t.suggestion}
              </button>
            )}
            <button className="btn btn-small" onClick={() => onOpenSettings(HOTKEY_FIELDS[t.action].field)}>
              Chọn phím khác
            </button>
          </div>
        ))}
        {error && <p className="status err">{error}</p>}
      </div>
      <button className="hotkey-warning-close" title="Ẩn thông báo" aria-label="Ẩn thông báo" onClick={() => setDismissed(key)}>
        ✕
      </button>
    </div>
  );
}
