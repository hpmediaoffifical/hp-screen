// Typed wrappers around the core's commands and events. Shapes mirror the Rust structs.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type Target =
  | { kind: "monitor"; index: number }
  | { kind: "window"; hwnd: number }
  | { kind: "region"; monitor: number; rect: Rect };

export interface MonitorInfo {
  index: number;
  name: string;
  rect: Rect;
  primary: boolean;
}

export interface WindowInfo {
  hwnd: number;
  title: string;
  process: string;
  rect: Rect;
}

export const listMonitors = () => invoke<MonitorInfo[]>("list_monitors");
export const listWindows = () => invoke<WindowInfo[]>("list_windows");
export const takeScreenshot = (target: Target) => invoke<string>("take_screenshot", { target });

export const onLibraryChanged = (handler: () => void): Promise<UnlistenFn> =>
  listen("library-changed", () => handler());

/** Commands reject with a Vietnamese message string; anything else gets a generic one. */
export const errorMessage = (e: unknown): string => {
  if (typeof e === "string") return e;
  // Show what actually went wrong instead of a generic message.
  if (e instanceof Error && e.message) return `Đã xảy ra lỗi: ${e.message}`;
  return "Đã xảy ra lỗi không xác định.";
};

export type RecState = "idle" | "countdown" | "recording" | "paused" | "finalizing";

export interface RecStatus {
  state: RecState;
  elapsed_ms: number;
  countdown: number | null;
  file: string | null;
  warning: string | null;
}

export const recStart = (target: Target) => invoke<void>("rec_start", { target });
export const recStop = () => invoke<void>("rec_stop");
export const recTogglePause = () => invoke<void>("rec_toggle_pause");
export const recCancel = () => invoke<void>("rec_cancel");
export const recStatus = () => invoke<RecStatus>("rec_status");

export const onRecState = (handler: (s: RecStatus) => void): Promise<UnlistenFn> =>
  listen<RecStatus>("rec-state", (e) => handler(e.payload));
export const onRecError = (handler: (message: string) => void): Promise<UnlistenFn> =>
  listen<{ message: string }>("rec-error", (e) => handler(e.payload.message));

/** 75_000 ms → "01:15"; an hour or more → "1:02:03". */
export function formatElapsed(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = String(m).padStart(2, "0");
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

export interface AudioStatus {
  system_available: boolean;
  mic_available: boolean;
}
export const audioStatus = () => invoke<AudioStatus>("audio_status");

export type PickMode = "region" | "window";
export interface PickerContext {
  mode: PickMode;
  monitor: MonitorInfo;
  windows: WindowInfo[];
}
export const pickTarget = (mode: PickMode) => invoke<Target | null>("pick_target", { mode });
export const pickerContext = () => invoke<PickerContext | null>("picker_context");
export const pickerDone = (target: Target | null) => invoke<void>("picker_done", { target });

export type Quality = "light" | "standard" | "high";
export interface Settings {
  version: number;
  output_dir: string | null;
  name_pattern: string;
  fps: number;
  quality: Quality;
  countdown_secs: number;
  system_audio: boolean;
  mic: boolean;
  hotkey_toggle: string;
  hotkey_pause: string;
  hotkey_screenshot: string;
  hotkey_quickshot: string;
  input_log: boolean;
  autostart: boolean;
  last_region: Rect | null;
  hotkey_draw: string;
  /** 0, 2, 3, 5 or 10; 0 keeps strokes until cleared. */
  draw_fade_secs: number;
}
export const settingsGet = () => invoke<Settings>("settings_get");
export const settingsSet = (settings: Settings) => invoke<Settings>("settings_set", { settings });

export interface HotkeyStatus {
  action: "toggle" | "pause" | "screenshot" | "quickshot" | "draw";
  hotkey: string;
  ok: boolean;
  message: string | null;
}
export const hotkeyStatus = () => invoke<HotkeyStatus[]>("hotkey_status");

/** Smallest region side the recorder accepts, in physical pixels (mirrors hp-core MIN_SIZE). */
export const MIN_REGION = 64;

export type MediaKind = "video" | "image";
export interface LibraryEntry {
  path: string;
  name: string;
  kind: MediaKind;
  created_ms: number;
  size: number;
  thumb: string | null;
  duration_ms: number | null;
  width: number | null;
  height: number | null;
}
export const libraryList = (filter: MediaKind | null) => invoke<LibraryEntry[]>("library_list", { filter });
export const libraryRename = (path: string, newStem: string) => invoke<string>("library_rename", { path, newStem });
export const libraryDelete = (path: string) => invoke<void>("library_delete", { path });
export const libraryOpen = (path: string) => invoke<void>("library_open", { path });
export const libraryReveal = (path: string | null) => invoke<void>("library_reveal", { path });

/** 1536 → "1,5 KB"; sizes use Vietnamese decimal commas. */
export function formatSize(bytes: number): string {
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const text = unit === 0 ? String(value) : value.toFixed(1).replace(".", ",");
  return `${text} ${units[unit]}`;
}

export const previewName = (pattern: string) => invoke<string>("preview_name", { pattern });
export const pickFolder = () => invoke<string | null>("pick_folder");
export const outputFolder = () => invoke<string>("output_folder");

export const setTarget = (target: Target) => invoke<void>("set_target", { target });

export type SnipAction = "copy" | "save" | "both";
export interface SnipContext {
  frozen: string;
  monitor: MonitorInfo;
}
/** Opens "capture and annotate" from the main window. */
export const snipStart = () => invoke<void>("snip_start");
export const snipContext = () => invoke<SnipContext | null>("snip_context");
export const snipReady = () => invoke<void>("snip_ready");
export const snipFinish = (action: SnipAction, png: string) => invoke<string | null>("snip_finish", { action, png });
export const snipCancel = () => invoke<void>("snip_cancel");

export const compactShow = () => invoke<void>("compact_show");
export const compactHide = () => invoke<void>("compact_hide");
export const openMain = () => invoke<void>("open_main");
export const getTarget = () => invoke<Target>("get_target");
export const onTargetChanged = (handler: (t: Target) => void): Promise<UnlistenFn> =>
  listen<Target>("target-changed", (e) => handler(e.payload));
export const onSettingsChanged = (handler: (s: Settings) => void): Promise<UnlistenFn> =>
  listen<Settings>("settings-changed", (e) => handler(e.payload));
/** The frozen screen's image bytes (BMP). */
export const snipFrozen = () => invoke<ArrayBuffer>("snip_frozen");
