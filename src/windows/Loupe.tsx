/*! © 2026 HP Media (hpvn.media) — HP Screen. Bảo lưu mọi quyền. HP không có đại lý khác, không hợp tác với đơn vị nào, mọi sao chép đều là vi phạm. */
import { useEffect, useRef } from "react";
import { loupePlace, loupeSource, LOUPE_CELLS, LOUPE_ZOOM, type Box, type Point } from "./loupeMath";

const SIZE = LOUPE_CELLS * LOUPE_ZOOM;
/** Height of the coordinate line under the picture, CSS pixels. */
const LABEL = 22;

/**
 * A small magnified view of the screen around `at`, shown beside the pointer while an area is
 * chosen or resized: each screen pixel is drawn large, the area's edge is drawn on top, and
 * what stays outside the area is darkened, so it is plain which pixels are in and which are out.
 *
 * `image` is the screen at physical resolution; `at` and `box` are in its pixels; `scale` is
 * image pixels per CSS pixel.
 */
export default function Loupe({ image, at, box, scale }: { image: HTMLImageElement; at: Point; box: Box | null; scale: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const src = loupeSource(at, image.naturalWidth, image.naturalHeight);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d")!;
    ctx.imageSmoothingEnabled = false;
    ctx.fillStyle = "#000";
    ctx.fillRect(0, 0, SIZE, SIZE);
    ctx.drawImage(image, src.x, src.y, LOUPE_CELLS, LOUPE_CELLS, 0, 0, SIZE, SIZE);

    if (box && box.w > 0 && box.h > 0) {
      // The area as it will be cut out: whole pixels.
      const x0 = (Math.round(box.x) - src.x) * LOUPE_ZOOM;
      const y0 = (Math.round(box.y) - src.y) * LOUPE_ZOOM;
      const x1 = (Math.round(box.x + box.w) - src.x) * LOUPE_ZOOM;
      const y1 = (Math.round(box.y + box.h) - src.y) * LOUPE_ZOOM;
      ctx.save();
      ctx.beginPath();
      ctx.rect(0, 0, SIZE, SIZE);
      ctx.rect(x0, y0, x1 - x0, y1 - y0);
      ctx.fillStyle = "rgba(0, 0, 0, 0.5)";
      ctx.fill("evenodd");
      ctx.restore();
      ctx.strokeStyle = "#4c8dff";
      ctx.lineWidth = 2;
      ctx.strokeRect(x0, y0, x1 - x0, y1 - y0);
    }

    // The pixel under the point, and a faint cross to it.
    const c = (src.cx - src.x) * LOUPE_ZOOM;
    const r = (src.cy - src.y) * LOUPE_ZOOM;
    ctx.fillStyle = "rgba(255, 255, 255, 0.35)";
    ctx.fillRect(c + LOUPE_ZOOM / 2 - 0.5, 0, 1, SIZE);
    ctx.fillRect(0, r + LOUPE_ZOOM / 2 - 0.5, SIZE, 1);
    ctx.strokeStyle = "#fff";
    ctx.lineWidth = 1;
    ctx.strokeRect(c + 0.5, r + 0.5, LOUPE_ZOOM - 1, LOUPE_ZOOM - 1);
  }, [image, src.x, src.y, src.cx, src.cy, box?.x, box?.y, box?.w, box?.h]);

  const place = loupePlace(
    { x: at.x / scale, y: at.y / scale },
    box ? { x: (box.x + box.w / 2) / scale, y: (box.y + box.h / 2) / scale } : null,
    SIZE,
    SIZE + LABEL,
    window.innerWidth,
    window.innerHeight,
  );

  return (
    <div className="loupe" style={{ left: place.x, top: place.y, width: SIZE }} aria-hidden="true">
      <canvas ref={ref} width={SIZE} height={SIZE} />
      <span className="loupe-label">
        {src.cx}, {src.cy}
        {box && box.w > 0 && box.h > 0 ? ` · ${Math.round(box.w)} × ${Math.round(box.h)}` : ""}
      </span>
    </div>
  );
}
