import { useEffect, useState } from "react";
import { settingsGet, type CamShape } from "../api";
import { onCamFrame } from "../liveApi";

/**
 * The webcam bubble on screen, so the presenter sees themself. It is excluded from capture
 * and lets the mouse through; the picture in the video is drawn by the core.
 */
export default function CamBubble() {
  const [src, setSrc] = useState<string | null>(null);
  const [shape, setShape] = useState<CamShape>("circle");

  useEffect(() => {
    settingsGet().then((s) => setShape(s.webcam.shape)).catch(() => {});
    const off = onCamFrame((jpeg) => setSrc(`data:image/jpeg;base64,${jpeg}`));
    return () => void off.then((u) => u());
  }, []);

  return (
    <div className={"cam cam-" + shape}>
      {src ? <img className="cam-img" src={src} alt="" /> : <span className="cam-wait">Đang mở camera…</span>}
    </div>
  );
}
