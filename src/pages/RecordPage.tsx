import { useState } from "react";
import { errorMessage, takeScreenshot } from "../api";

export default function RecordPage() {
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);

  // Temporary control until the full record page lands (plan task 12).
  async function shoot() {
    setBusy(true);
    try {
      const path = await takeScreenshot({ kind: "monitor", index: 0 });
      setStatus(`Đã lưu ảnh và copy vào clipboard: ${path}`);
    } catch (e) {
      setStatus(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="page">
      <h1>Quay</h1>
      <button className="btn" onClick={shoot} disabled={busy}>
        Chụp ảnh màn hình
      </button>
      <p className="status" role="status">{status}</p>
    </section>
  );
}
