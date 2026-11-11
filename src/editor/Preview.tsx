import { convertFileSrc } from "@tauri-apps/api/core";
import { forwardRef, useEffect, useImperativeHandle, useRef, type ReactNode } from "react";
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
  /** Drawn over the video, same size as the picture (effects canvas). */
  overlay?: ReactNode;
}

/** The video with playback that jumps over removed spans and stops at the trim end. */
const Preview = forwardRef<PreviewHandle, Props>(function Preview(props, ref) {
  const video = useRef<HTMLVideoElement>(null);
  const keep = useRef(props.keep);
  keep.current = props.keep;
  const onTime = useRef(props.onTime);
  onTime.current = props.onTime;

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
    };
    const supportsFrames = "requestVideoFrameCallback" in v;
    const loop = () => {
      if (stopped) return;
      check();
      handle = supportsFrames ? v.requestVideoFrameCallback(loop) : requestAnimationFrame(loop);
    };
    loop();
    const onSeeked = () => onTime.current(v.currentTime * 1000);
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
        {props.overlay}
      </div>
    </div>
  );
});

export default Preview;
