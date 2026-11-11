import { convertFileSrc } from "@tauri-apps/api/core";
import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { keptTime, type Span } from "../editApi";

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
  /** Draws over the picture at source time `ms`; the canvas covers the whole video. */
  draw?: (ctx: CanvasRenderingContext2D, ms: number) => void;
}

/** The video with playback that jumps over removed spans and stops at the trim end. */
const Preview = forwardRef<PreviewHandle, Props>(function Preview(props, ref) {
  const video = useRef<HTMLVideoElement>(null);
  const keep = useRef(props.keep);
  keep.current = props.keep;
  const onTime = useRef(props.onTime);
  onTime.current = props.onTime;
  const canvas = useRef<HTMLCanvasElement>(null);
  const draw = useRef(props.draw);
  draw.current = props.draw;
  const repaint = useRef<() => void>(() => {});
  // Redraw when what is drawn changes, also while paused (no frames arrive then).
  useEffect(() => repaint.current(), [props.draw]);

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
    // The overlay canvas follows the size the video is shown at, in device pixels.
    const paint = () => {
      const c = canvas.current;
      const ctx = c?.getContext("2d");
      if (!c || !ctx) return;
      const w = Math.round(v.clientWidth * devicePixelRatio);
      const h = Math.round(v.clientHeight * devicePixelRatio);
      if (c.width !== w || c.height !== h) {
        c.width = w;
        c.height = h;
      }
      if (draw.current) draw.current(ctx, v.currentTime * 1000);
      else ctx.clearRect(0, 0, w, h);
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
    return () => {
      stopped = true;
      if (supportsFrames) v.cancelVideoFrameCallback(handle);
      else cancelAnimationFrame(handle);
      v.removeEventListener("seeked", onSeeked);
    };
  }, []);

  useImperativeHandle(ref, () => ({
    play() {
      const v = video.current;
      if (!v) return;
      const ms = v.currentTime * 1000;
      const target = keptTime(keep.current, ms) ?? keep.current[0]?.start_ms ?? 0;
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

  return (
    <div className="preview">
      <div className="preview-stage">
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
    </div>
  );
});

export default Preview;
