// Typed wrappers around the edit page's commands and events. Shapes mirror the Rust structs
// in src-tauri/src/edit_cmds.rs and crates/hp-core/src/edit.rs.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Half-open time span `[start_ms, end_ms)` in source milliseconds. */
export interface Span {
  start_ms: number;
  end_ms: number;
}

export interface EffectToggles {
  clicks: boolean;
  keys: boolean;
}

export interface ZoomSegment {
  start_ms: number;
  end_ms: number;
  scale: number;
  /** Fixed centre (0–1 of the frame); null follows the cursor, or stays centred. */
  center: [number, number] | null;
  /** Added or changed by hand: "Tự tạo lại" keeps it. */
  manual: boolean;
}

export interface ZoomDoc {
  enabled: boolean;
  segments: ZoomSegment[];
}

/** Where the zoom camera looks: centre (0–1) and zoom level. */
export interface Camera {
  cx: number;
  cy: number;
  scale: number;
}

export interface EditDoc {
  v: number;
  trim: { start_ms: number; end_ms: number | null };
  /** Removed spans, in source time. */
  cuts: Span[];
  effects: EffectToggles;
  zoom: ZoomDoc;
}

/** The video's size and length, sent with zoom commands. */
export interface Frame {
  width: number;
  height: number;
  duration_ms: number;
}

export type MouseButton = "left" | "right" | "middle";

/** Effects from the input log (hp-core::effects::Effects); positions in source pixels. */
export interface Effects {
  ripples: { t_ms: number; x: number; y: number; button: MouseButton }[];
  holds: { start_ms: number; end_ms: number; button: MouseButton }[];
  wheels: { start_ms: number; end_ms: number; up: boolean }[];
  chips: { start_ms: number; end_ms: number; text: string }[];
  /** [ms, x, y] */
  cursor: [number, number, number][];
}

export interface VideoInfo {
  width: number;
  height: number;
  duration_ms: number;
  fps: number;
  has_audio: boolean;
}

export interface EditSession {
  info: VideoInfo;
  doc: EditDoc;
  has_log: boolean;
  effects: Effects | null;
  warning: string | null;
}

export interface KeepInfo {
  keep: Span[];
  output_ms: number;
  can_export: boolean;
}

export const editOpen = (path: string) => invoke<EditSession>("edit_open", { path });
export const editSave = (path: string, doc: EditDoc) => invoke<void>("edit_save", { path, doc });
export const editKeep = (doc: EditDoc, durationMs: number) => invoke<KeepInfo>("edit_keep", { doc, durationMs });
export const exportStart = (path: string, doc: EditDoc) => invoke<string>("export_start", { path, doc });
export const exportCancel = () => invoke<void>("export_cancel");
export const zoomAuto = (path: string, doc: EditDoc, frame: Frame) => invoke<ZoomSegment[]>("zoom_auto", { path, doc, frame });
export const zoomTrack = (path: string, doc: EditDoc, frame: Frame) => invoke<[number, Camera][]>("zoom_track", { path, doc, frame });

// Every export event names the source video, so a page only follows its own export.
export const onExportProgress = (handler: (source: string, done: number, total: number) => void): Promise<UnlistenFn> =>
  listen<{ source: string; done: number; total: number }>("export-progress", (e) => handler(e.payload.source, e.payload.done, e.payload.total));
export const onExportDone = (handler: (source: string, path: string) => void): Promise<UnlistenFn> =>
  listen<{ source: string; path: string }>("export-done", (e) => handler(e.payload.source, e.payload.path));
export const onExportError = (handler: (source: string, message: string) => void): Promise<UnlistenFn> =>
  listen<{ source: string; message: string }>("export-error", (e) => handler(e.payload.source, e.payload.message));

export { addCut, keptTime, playFrom } from "./editor/timeMath.ts";

/** "01:15,4": minutes, seconds and tenths, for timeline labels. */
export function formatPrecise(ms: number): string {
  const t = Math.max(0, Math.round(ms / 100));
  const tenths = t % 10;
  const s = Math.floor(t / 10) % 60;
  const m = Math.floor(t / 600);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")},${tenths}`;
}
