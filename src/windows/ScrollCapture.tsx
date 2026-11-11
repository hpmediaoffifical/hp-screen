import { useEffect, useState } from "react";
import { errorMessage, onScrollProgress, scrollClose, snipScrollStop, type ScrollProgress } from "../api";

/** Click-through frame around the area being captured while the user scrolls it. */
export function ScrollFrame() {
  return <div className="scroll-frame" />;
}

/** Small bar beside the area: what to do, how tall the picture is, and Done / Cancel. */
export function ScrollBar() {
  const [progress, setProgress] = useState<ScrollProgress | null>(null);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    const unlisten = onScrollProgress(setProgress);
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const finish = async (keep: boolean) => {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      const result = await snipScrollStop(keep);
      if (!keep || !result) return scrollClose();
      setDone(`✓ Đã lưu và sao chép ảnh dài ${result[1]} × ${result[2]} px`);
      window.setTimeout(() => scrollClose(), 1800);
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  };

  const height = progress ? progress.height.toLocaleString("vi-VN") : "0";
  let message = `Lăn chuột trong khung xanh để cuộn trang xuống · Ảnh đang cao ${height} px`;
  if (progress?.status === "lost") message = "Chưa khớp với phần đã chụp: cuộn ngược lên một chút rồi cuộn xuống chậm hơn. Đoạn trống quá dài cũng không ghép được.";
  if (progress?.status === "full") message = `Ảnh đã cao tối đa (${height} px). Bấm Xong để lưu.`;
  if (error) message = error;
  if (done) message = done;

  return (
    <div className="scroll-bar" role="status">
      <span className={"scroll-msg" + (progress?.status === "lost" || error ? " warn" : "")}>{message}</span>
      {!done && (
        <>
          <button className="snip-btn snip-wide" disabled={busy} onClick={() => finish(false)}>
            Hủy
          </button>
          <button className="snip-btn snip-wide snip-done" disabled={busy || !progress} onClick={() => finish(true)}>
            {busy ? "Đang lưu…" : "✓ Xong"}
          </button>
        </>
      )}
    </div>
  );
}
