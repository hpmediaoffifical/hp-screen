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

/** A keyframe inside a zoom segment (hp-core::zoom::ZoomKey): from here the camera aims at this frame. */
export interface ZoomKey {
  t_ms: number;
  cx: number;
  cy: number;
  scale: number;
}

export type Speed = "slow" | "normal" | "fast" | "instant";
export type Level = "low" | "normal" | "high";

export interface ZoomSegment {
  start_ms: number;
  end_ms: number;
  scale: number;
  /** Fixed centre (0–1 of the frame); null follows the cursor, or stays centred. */
  center: [number, number] | null;
  /** Added or changed by hand: "Tự tạo lại" keeps it. */
  manual: boolean;
  /** Keyframes after the start (the start is `center` and `scale`). */
  keys: ZoomKey[];
  speed: Speed;
}

export interface ZoomDoc {
  enabled: boolean;
  segments: ZoomSegment[];
  /** Level of automatic zoom. */
  level: Level;
  /** Where automatic zoom works (dropped on the zoom track); none: no automatic zoom. */
  regions: Span[];
}

/** Where the zoom camera looks: centre (0–1) and zoom level. */
export interface Camera {
  cx: number;
  cy: number;
  scale: number;
}

export type Aspect = "source" | "16:9" | "9:16" | "1:1";
export type Fit = "fit" | "fill";

export interface FrameDoc {
  aspect: Aspect;
  fit: Fit;
  background: string;
  /** Fraction of the short side, each side (0–0.2). */
  padding: number;
  /** Corner radius in output pixels (0–40). */
  radius: number;
  shadow: boolean;
}

export type Background = { kind: "solid"; color: [number, number, number] } | { kind: "gradient"; from: [number, number, number]; to: [number, number, number] };

/** hp-core::layout::Layout */
export interface Layout {
  out_w: number;
  out_h: number;
  video: [number, number, number, number];
  radius: number;
  shadow: boolean;
  background: Background;
  fill_aspect: number | null;
  decorated: boolean;
}

export interface GifOptions {
  fps: number;
  width: number;
}

/** A subtitle line, in source time. `text` may hold a line break. */
export interface Cue {
  start_ms: number;
  end_ms: number;
  text: string;
}

export interface SubtitleDoc {
  cues: Cue[];
  /** Burned into exported videos. */
  burn: boolean;
}

export interface EditDoc {
  v: number;
  trim: { start_ms: number; end_ms: number | null };
  /** Removed spans, in source time. */
  cuts: Span[];
  effects: EffectToggles;
  zoom: ZoomDoc;
  frame: FrameDoc;
  subtitles: SubtitleDoc;
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
export const exportStart = (path: string, doc: EditDoc, gif: GifOptions | null = null) => invoke<string>("export_start", { path, doc, gif });
export const editLayout = (doc: EditDoc, frame: Frame) => invoke<Layout>("edit_layout", { doc, frame });
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

export interface AsrModel {
  id: string;
  name: string;
  size_mb: number;
  installed: boolean;
}

export interface AsrStatus {
  /** whisper.cpp is installed. */
  cli: boolean;
  cli_size_mb: number;
  models: AsrModel[];
  /** Where it is installed. */
  folder: string;
}

export const asrStatus = () => invoke<AsrStatus>("asr_status");
export const asrInstall = (model: string) => invoke<void>("asr_install", { model });
export const asrCancel = () => invoke<void>("asr_cancel");
export const subtitlesGenerate = (path: string, model: string) => invoke<void>("subtitles_generate", { path, model });
export const exportSrt = (path: string, doc: EditDoc, durationMs: number) => invoke<string>("export_srt", { path, doc, durationMs });

export const onAsrProgress = (handler: (phase: "cli" | "model", done: number, total: number) => void): Promise<UnlistenFn> =>
  listen<{ phase: "cli" | "model"; done: number; total: number }>("asr-progress", (e) => handler(e.payload.phase, e.payload.done, e.payload.total));
export const onAsrDone = (handler: () => void): Promise<UnlistenFn> => listen("asr-done", () => handler());
export const onAsrError = (handler: (message: string) => void): Promise<UnlistenFn> =>
  listen<{ message: string }>("asr-error", (e) => handler(e.payload.message));
export const onSubsProgress = (handler: (source: string, phase: "audio" | "recognise", percent: number) => void): Promise<UnlistenFn> =>
  listen<{ source: string; phase: "audio" | "recognise"; percent: number }>("subs-progress", (e) => handler(e.payload.source, e.payload.phase, e.payload.percent));
export const onSubsDone = (handler: (source: string, cues: Cue[]) => void): Promise<UnlistenFn> =>
  listen<{ source: string; cues: Cue[] }>("subs-done", (e) => handler(e.payload.source, e.payload.cues));
export const onSubsError = (handler: (source: string, message: string) => void): Promise<UnlistenFn> =>
  listen<{ source: string; message: string }>("subs-error", (e) => handler(e.payload.source, e.payload.message));

export { addCut, keptTime, playFrom } from "./editor/timeMath.ts";

/** "01:15,4": minutes, seconds and tenths, for timeline labels. */
export function formatPrecise(ms: number): string {
  const t = Math.max(0, Math.round(ms / 100));
  const tenths = t % 10;
  const s = Math.floor(t / 10) % 60;
  const m = Math.floor(t / 600);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")},${tenths}`;
}
