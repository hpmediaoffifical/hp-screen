// Typed wrappers around the live drawing (and webcam) commands. Shapes mirror the Rust structs
// in crates/hp-core/src/stroke.rs and src-tauri/src/live_cmds.rs.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Tool = "pen" | "arrow" | "rect";

export interface Stroke {
  id: number;
  tool: Tool;
  color: [number, number, number];
  width: number;
  /** Physical pixels relative to the recorded area. */
  points: [number, number][];
  /** Send any number to mark the stroke finished; the core stamps the real time. */
  finished_ms: number | null;
}

export interface DrawContext {
  on: boolean;
  hold_ms: number;
}

export const drawSet = (on: boolean) => invoke<void>("draw_set", { on });
export const drawToggle = () => invoke<void>("draw_toggle");
export const drawStroke = (stroke: Stroke) => invoke<void>("draw_stroke", { stroke });
export const drawClear = () => invoke<void>("draw_clear");
export const drawContext = () => invoke<DrawContext>("draw_context");
export const onDrawMode = (handler: (on: boolean) => void): Promise<UnlistenFn> =>
  listen<{ on: boolean }>("draw-mode", (e) => handler(e.payload.on));
