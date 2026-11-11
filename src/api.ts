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
export const errorMessage = (e: unknown): string =>
  typeof e === "string" ? e : "Đã xảy ra lỗi không xác định.";

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
