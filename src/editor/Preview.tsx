import { convertFileSrc } from "@tauri-apps/api/core";
import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import { keptTime, playFrom, type Camera, type Layout, type Span } from "../editApi";
import { gradientAngle, pickInView, sourceView, type View } from "./layoutMath";
import { WHOLE } from "./zoomMath";
import ZoomFrame from "./ZoomFrame";

export interface PreviewHandle {
  play(): void;
  pause(): void;
  toggle(): void;
  seek(ms: number): void;
  isPlaying(): boolean;
}

interface Props {
  path: string;
  /** Source size in pixels. */
  srcW: number;
  srcH: number;
  keep: Span[];
  /** The output frame (aspect, background, where the video sits); null is the plain video. */
  layout: Layout | null;
  onTime: (ms: number) => void;
  onPlaying: (playing: boolean) => void;
  onError: () => void;
  /** Draws effects that move with the picture (rings), at source time `ms`. */
  draw?: (ctx: CanvasRenderingContext2D, ms: number) => void;
  /** Draws what stays put in the frame (shortcut chips), at source time `ms`. */
  drawFixed?: (ctx: CanvasRenderingContext2D, ms: number) => void;
  /** The zoom camera at source time `ms`. */
  camera?: (ms: number) => Camera;
  /** When set, a click on the picture picks that point of the source frame (0–1). */
  onPick?: (x: number, y: number) => void;
  /** When set, the picture is shown unzoomed with this zoom frame to drag over it. */
  frameEdit?: { camera: Camera; onChange: (cam: Camera, first: boolean) => void };
}

const css = (c: [number, number, number]) => `rgb(${c[0]}, ${c[1]}, ${c[2]})`;

/**
 * The video as it will be exported: in its frame and background, cropped and zoomed like the
 * export, with effect layers on top. Playback jumps over removed spans and stops at the trim end.
 */
const Preview = forwardRef<PreviewHandle, Props>(function Preview(props, ref) {
  const box = useRef<HTMLDivElement>(null);
  const win = useRef<HTMLDivElement>(null);
  const video = useRef<HTMLVideoElement>(null);
  const zoom = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const fixed = useRef<HTMLCanvasElement>(null);
  const [frame, setFrame] = useState({ w: 0, h: 0 });

  // Everything the per-frame loop reads, kept current without restarting it.
  const live = useRef(props);
  live.current = props;
  const view = useRef<View>([0, 0, props.srcW, props.srcH]);
  const repaint = useRef<() => void>(() => {});
  useEffect(() => repaint.current(), [props.draw, props.drawFixed, props.camera, props.layout, props.frameEdit, frame]);

  // While the zoom frame is edited the preview shows the plain, whole source.
  const shownLayout = props.frameEdit ? null : props.layout;
  const outW = shownLayout?.out_w ?? props.srcW;
  const outH = shownLayout?.out_h ?? props.srcH;

  // The frame is as large as fits the preview area, at the output aspect.
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const fit = () => {
      const { width, height } = el.getBoundingClientRect();
      if (width <= 0 || height <= 0 || outW <= 0 || outH <= 0) return;
      const k = Math.min(width / outW, height / outH);
      setFrame({ w: Math.floor(outW * k), h: Math.floor(outH * k) });
    };
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
  }, [outW, outH]);

  useEffect(() => {
    const v = video.current;
    if (!v) return;
    let handle = 0;
    let stopped = false;
    const check = () => {
      const p = live.current;
      const ms = v.currentTime * 1000;
      if (!v.paused) {
        const target = keptTime(p.keep, ms);
        if (target === null) {
          v.pause();
          const last = p.keep[p.keep.length - 1];
          if (last) v.currentTime = Math.max(0, last.end_ms - 1) / 1000;
        } else if (target - ms > 1) {
          v.currentTime = target / 1000;
        }
      }
      p.onTime(v.currentTime * 1000);
      paint();
    };
    const fit = (c: HTMLCanvasElement | null, w: number, h: number) => {
      const ctx = c?.getContext("2d");
      if (!c || !ctx) return null;
      const pw = Math.max(1, Math.round(w * devicePixelRatio));
      const ph = Math.max(1, Math.round(h * devicePixelRatio));
      if (c.width !== pw || c.height !== ph) {
        c.width = pw;
        c.height = ph;
      }
      return ctx;
    };
    const paint = () => {
      const p = live.current;
      const w = win.current;
      if (!w || p.srcW <= 0 || p.srcH <= 0) return;
      const ms = v.currentTime * 1000;
      const cam = p.camera ? p.camera(ms) : WHOLE;
      const fill = p.frameEdit ? null : (p.layout?.fill_aspect ?? null);
      const now = sourceView(cam, p.srcW, p.srcH, fill);
      const base = sourceView(WHOLE, p.srcW, p.srcH, fill);
      view.current = now;
      // The element is the whole source at the scale where the unzoomed view fills the window;
      // zoom and crop are a transform on top, so the picture matches the export.
      const ww = w.clientWidth;
      const k1 = ww / base[2];
      const ew = p.srcW * k1;
      const eh = p.srcH * k1;
      for (const el of [v, canvas.current]) {
        if (el) {
          el.style.width = `${ew}px`;
          el.style.height = `${eh}px`;
        }
      }
      if (zoom.current) {
        const s = base[2] / now[2];
        zoom.current.style.transform = `scale(${s}) translate(${-now[0] * k1}px, ${-now[1] * k1}px)`;
      }
      const ctx = fit(canvas.current, ew, eh);
      if (ctx) {
        if (p.draw) p.draw(ctx, ms);
        else ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
      }
      const b = box.current?.querySelector(".preview-frame") as HTMLElement | null;
      const fctx = fit(fixed.current, b?.clientWidth ?? 0, b?.clientHeight ?? 0);
      if (fctx) {
        if (p.drawFixed) p.drawFixed(fctx, ms);
        else fctx.clearRect(0, 0, fctx.canvas.width, fctx.canvas.height);
      }
    };
    repaint.current = paint;
    const supportsFrames = "requestVideoFrameCallback" in v;
    const loop = () => {
      if (stopped) return;
      check();
      handle = supportsFrames ? v.requestVideoFrameCallback(loop) : requestAnimationFrame(loop);
    };
    loop();
    const onSeeked = () => {
      live.current.onTime(v.currentTime * 1000);
      paint();
    };
    v.addEventListener("seeked", onSeeked);
    v.addEventListener("loadeddata", paint);
    return () => {
      stopped = true;
      if (supportsFrames) v.cancelVideoFrameCallback(handle);
      else cancelAnimationFrame(handle);
      v.removeEventListener("seeked", onSeeked);
      v.removeEventListener("loadeddata", paint);
    };
  }, []);

  useImperativeHandle(ref, () => ({
    play() {
      const v = video.current;
      if (!v) return;
      const ms = v.currentTime * 1000;
      const target = playFrom(live.current.keep, ms);
      if (Math.abs(target - ms) > 1) v.currentTime = target / 1000;
      void v.play().catch(() => {});
    },
    pause() {
      video.current?.pause();
    },
    toggle() {
      if (video.current?.paused) this.play();
      else this.pause();
    },
    seek(ms: number) {
      if (video.current) video.current.currentTime = Math.max(0, ms) / 1000;
    },
    isPlaying() {
      return !!video.current && !video.current.paused;
    },
  }));

  const pick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!props.onPick) return;
    const r = e.currentTarget.getBoundingClientRect();
    const [x, y] = pickInView(view.current, e.clientX - r.left, e.clientY - r.top, r.width, r.height, props.srcW, props.srcH);
    props.onPick(x, y);
  };

  const l = shownLayout;
  const k = frame.w > 0 ? frame.w / outW : 0;
  const [vx, vy, vw, vh] = l ? l.video : [0, 0, outW, outH];
  const background = !l || !l.decorated ? "#000" : l.background.kind === "solid" ? css(l.background.color) : `linear-gradient(${gradientAngle(outW, outH)}deg, ${css(l.background.from)}, ${css(l.background.to)})`;
  const shadow = l?.shadow ? `0 ${0.015 * Math.min(outW, outH) * k}px ${0.04 * Math.min(outW, outH) * k}px rgba(0, 0, 0, 0.45)` : "none";

  return (
    <div className="preview" ref={box}>
      <div className="preview-frame" style={{ width: frame.w, height: frame.h, background }}>
        <div
          ref={win}
          className={"preview-stage" + (props.onPick ? " preview-picking" : "")}
          style={{ left: vx * k, top: vy * k, width: vw * k, height: vh * k, borderRadius: (l?.radius ?? 0) * k, boxShadow: shadow }}
          onClick={pick}
        >
          <div ref={zoom} className="preview-zoom">
            <video
              ref={video}
              className="preview-video"
              src={convertFileSrc(props.path)}
              preload="auto"
              onPlay={() => props.onPlaying(true)}
              onPause={() => props.onPlaying(false)}
              onError={props.onError}
            />
            <canvas ref={canvas} className="preview-overlay" aria-hidden="true" />
          </div>
          {props.frameEdit && (
            <ZoomFrame camera={props.frameEdit.camera} srcW={props.srcW} srcH={props.srcH} fill={props.layout?.fill_aspect ?? null} onChange={props.frameEdit.onChange} />
          )}
        </div>
        <canvas ref={fixed} className="preview-overlay" aria-hidden="true" />
      </div>
    </div>
  );
});

export default Preview;
