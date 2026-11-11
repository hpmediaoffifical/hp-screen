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
