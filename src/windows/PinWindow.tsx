import { useEffect, useRef, useState } from "react";
import { getCurrentWindow, PhysicalSize } from "@tauri-apps/api/window";
import { errorMessage, pinClose, pinCopy, pinImage, pinReady } from "../api";

/**
 * A pinned picture: a small borderless window that stays on top. Drag it anywhere, resize it
 * with the mouse wheel or the corner grip, double-click for its original size, Ctrl+C or the
 * button copies it, Esc or ✕ closes it.
 */
const MIN_WIDTH = 40;
const MAX_ZOOM = 5;

export default function PinWindow() {
  const [url, setUrl] = useState("");
  const natural = useRef<{ w: number; h: number } | null>(null);
  /** Width the window is being resized to (physical pixels), so fast wheel turns add up. */
  const width = useRef(0);
  const [zoom, setZoom] = useState<number | null>(null);
  const zoomTimer = useRef(0);
  const [note, setNote] = useState("");
  /** Where a press started; the window is dragged only once the pointer really moves, so a
   * click or a double-click never starts a window drag (which would swallow the second click). */
  const press = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    let objectUrl = "";
    pinImage()
      .then((bytes) => {
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        setUrl(objectUrl);
      })
      .catch(() => pinClose());
    return () => {
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, []);

  const copy = () =>
    pinCopy()
      .then(() => flash("✓ Đã sao chép"))
      .catch((e) => flash(errorMessage(e)));

  const flash = (text: string) => {
    setNote(text);
    window.setTimeout(() => setNote(""), 1600);
  };

  const resizeTo = (w: number) => {
    const n = natural.current;
    if (!n) return;
    const next = Math.round(Math.min(Math.max(w, MIN_WIDTH), n.w * MAX_ZOOM));
    width.current = next;
    getCurrentWindow()
      .setSize(new PhysicalSize(next, Math.max(1, Math.round((next * n.h) / n.w))))
      .catch(() => {});
    setZoom(Math.round((next / n.w) * 100));
    window.clearTimeout(zoomTimer.current);
    zoomTimer.current = window.setTimeout(() => setZoom(null), 900);
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") pinClose();
      else if (e.ctrlKey && e.key.toLowerCase() === "c") {
        e.preventDefault();
        copy();
      }
    };
    // A corner-grip resize changes the size behind our back; start the next wheel turn from it.
    const onResize = () => (width.current = Math.round(window.innerWidth * window.devicePixelRatio));
    window.addEventListener("keydown", onKey);
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", onResize);
    };
  }, []);

  return (
    <div
      className="pin"
      onWheel={(e) => {
        const from = width.current || Math.round(window.innerWidth * window.devicePixelRatio);
        resizeTo(from * (e.deltaY < 0 ? 1.1 : 1 / 1.1));
      }}
    >
      {url && (
        <img
          className="pin-image"
          src={url}
          alt="Ảnh đã ghim"
          draggable={false}
          onLoad={(e) => {
            natural.current = { w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight };
            width.current = Math.round(window.innerWidth * window.devicePixelRatio);
            requestAnimationFrame(() => pinReady().catch(() => {}));
          }}
          onPointerDown={(e) => {
            press.current = e.button === 0 ? { x: e.clientX, y: e.clientY } : null;
          }}
          onPointerMove={(e) => {
            const start = press.current;
            if (!start || !(e.buttons & 1)) return;
            if (Math.hypot(e.clientX - start.x, e.clientY - start.y) < 4) return;
            press.current = null;
            getCurrentWindow().startDragging().catch(() => {});
          }}
          onPointerUp={() => (press.current = null)}
          onDoubleClick={() => natural.current && resizeTo(natural.current.w)}
        />
      )}
      <div className="pin-controls">
        <button className="pin-btn" title="Sao chép ảnh (Ctrl+C)" aria-label="Sao chép ảnh" onClick={copy}>
          ⧉
        </button>
        <button className="pin-btn" title="Đóng (Esc)" aria-label="Đóng ảnh ghim" onClick={() => pinClose()}>
          ✕
        </button>
      </div>
      {(zoom !== null || note) && <span className="pin-note">{note || `${zoom}%`}</span>}
      <span
        className="pin-grip"
        title="Kéo để đổi cỡ"
        aria-hidden="true"
        onPointerDown={(e) => {
          e.preventDefault();
          getCurrentWindow().startResizeDragging("SouthEast").catch(() => {});
        }}
      />
    </div>
  );
}
