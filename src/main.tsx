/*! © 2026 HP Media (hpvn.media) — HP Screen. Bảo lưu mọi quyền. HP không có đại lý khác, không hợp tác với đơn vị nào, mọi sao chép đều là vi phạm. */
import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import ControlBar from "./windows/ControlBar";
import PickerOverlay from "./windows/PickerOverlay";
import SnipOverlay from "./windows/SnipOverlay";
import DrawOverlay from "./windows/DrawOverlay";
import CamBubble from "./windows/CamBubble";
import RegionBorder from "./windows/RegionBorder";
import PinWindow from "./windows/PinWindow";
import { ScrollBar, ScrollFrame } from "./windows/ScrollCapture";
import { pageFailed } from "./api";
import "./theme.css";

const describe = (e: unknown) => (e instanceof Error ? `${e.message}
${e.stack ?? ""}` : String(e)).slice(0, 2000);
// Nothing fails silently: what no code caught goes to the log.
window.addEventListener("error", (e) => pageFailed(describe(e.error ?? e.message), false));
window.addEventListener("unhandledrejection", (e) => pageFailed(describe(e.reason), false));

/** Catches a crash while drawing. React then removes the whole page, and for a full-screen
 * overlay that would leave an empty window on top of everything: the core closes it instead. */
class CrashGuard extends React.Component<{ children: React.ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  componentDidCatch(error: unknown) {
    pageFailed(describe(error), true);
  }
  render() {
    if (!this.state.failed) return this.props.children;
    return <p className="crash">HP Screen gặp lỗi khi hiển thị. Hãy đóng và mở lại cửa sổ này.</p>;
  }
}

// Every window loads the same page; the window label decides what it shows.
const label = getCurrentWindow().label;
document.documentElement.dataset.window = label;
const snipExtra =
  label.startsWith("pin-") ? <PinWindow /> : label === "scroll-bar" ? <ScrollBar /> : label === "scroll-frame" ? <ScrollFrame /> : null;
const root =
  snipExtra ??
  (label === "draw" ? <DrawOverlay /> : label === "cam" ? <CamBubble /> : label === "region-border" ? <RegionBorder /> : label === "control-bar" ? <ControlBar /> : label === "picker" ? <PickerOverlay /> : label === "snip" ? <SnipOverlay /> : <App />);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<React.StrictMode>
    <CrashGuard>{root}</CrashGuard>
  </React.StrictMode>,
);
