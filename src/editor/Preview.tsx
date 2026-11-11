import { convertFileSrc } from "@tauri-apps/api/core";
import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { keptTime, playFrom, type Camera, type Span } from "../editApi";
import { pickPoint, viewOffset, WHOLE } from "./zoomMath";

export interface PreviewHandle {
  play(): void;
  pause(): void;
  toggle(): void;
  seek(ms: number): void;
  isPlaying(): boolean;
}

interface Props {
  path: string;
  keep: Span[];
  onTime: (ms: number) => void;
  onPlaying: (playing: boolean) => void;
  onError: () => void;
  /** Draws effects that zoom with the picture (rings), at source time `ms`. */
  draw?: (ctx: CanvasRenderingContext2D, ms: number) => void;
  /** Draws what stays put in the frame (shortcut chips), at source time `ms`. */
  drawFixed?: (ctx: CanvasRenderingContext2D, ms: number) => void;
  /** The zoom camera at source time `ms`. */
  camera?: (ms: number) => Camera;
  /** When set, a click on the picture picks that point of the frame (0–1). */
  onPick?: (x: number, y: number) => void;
}

/**
 * The video with playback that jumps over removed spans and stops at the trim end, zoomed
 * like the export, with effect layers on top.
 */
const Preview = forwardRef<PreviewHandle, Props>(function Preview(props, ref) {
  const video = useRef<HTMLVideoElement>(null);
  const zoom = useRef<HTMLDivElement>(null);
  const keep = useRef(props.keep);
  keep.current = props.keep;
  const onTime = useRef(props.onTime);
  onTime.current = props.onTime;
  const canvas = useRef<HTMLCanvasElement>(null);
  const fixed = useRef<HTMLCanvasElement>(null);
  const draw = useRef(props.draw);
  draw.current = props.draw;
  const drawFixed = useRef(props.drawFixed);
  drawFixed.current = props.drawFixed;
  const camera = useRef(props.camera);
  camera.current = props.camera;
  const current = useRef<Camera>(WHOLE);
  const repaint = useRef<() => void>(() => {});
  // Redraw when what is drawn changes, also while paused (no frames arrive then).
  useEffect(() => repaint.current(), [props.draw, props.drawFixed, props.camera]);

  // Keep playback inside kept spans on every frame.
  useEffect(() => {
    const v = video.current;
    if (!v) return;
    let handle = 0;
    let stopped = false;
    const check = () => {
      const ms = v.currentTime * 1000;
      if (!v.paused) {
        const target = keptTime(keep.current, ms);
        if (target === null) {
          v.pause();
          const last = keep.current[keep.current.length - 1];
          if (last) v.currentTime = Math.max(0, last.end_ms - 1) / 1000;
        } else if (target - ms > 1) {
          v.currentTime = target / 1000;
        }
      }
      onTime.current(v.currentTime * 1000);
      paint();
    };
    // Layers follow the size the video is shown at, in device pixels.
    const fit = (c: HTMLCanvasElement | null) => {
      const ctx = c?.getContext("2d");
      if (!c || !ctx) return null;
      const w = Math.round(v.clientWidth * devicePixelRatio);
      const h = Math.round(v.clientHeight * devicePixelRatio);
      if (c.width !== w || c.height !== h) {
        c.width = w;
        c.height = h;
      }
      return ctx;
    };
    const paint = () => {
      const ms = v.currentTime * 1000;
      const cam = camera.current ? camera.current(ms) : WHOLE;
      current.current = cam;
      if (zoom.current) {
        const { s, x, y } = viewOffset(cam, v.clientWidth, v.clientHeight);
        zoom.current.style.transform = s > 1.0001 ? `scale(${s}) translate(${-x}px, ${-y}px)` : "";
      }
      const ctx = fit(canvas.current);
      if (ctx) {
        if (draw.current) draw.current(ctx, ms);
        else ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
      }
      const fctx = fit(fixed.current);
      if (fctx) {
        if (drawFixed.current) drawFixed.current(fctx, ms);
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
      onTime.current(v.currentTime * 1000);
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
      const target = playFrom(keep.current, ms);
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
    const [x, y] = pickPoint(current.current, e.clientX - r.left, e.clientY - r.top, r.width, r.height);
    props.onPick(x, y);
  };

  return (
    <div className="preview">
      <div className={"preview-stage" + (props.onPick ? " preview-picking" : "")} onClick={pick}>
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
        <canvas ref={fixed} className="preview-overlay" aria-hidden="true" />
      </div>
    </div>
  );
});

export default Preview;
