import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import ControlBar from "./windows/ControlBar";
import PickerOverlay from "./windows/PickerOverlay";
import SnipOverlay from "./windows/SnipOverlay";
import "./theme.css";

// Every window loads the same page; the window label decides what it shows.
const label = getCurrentWindow().label;
document.documentElement.dataset.window = label;
const root =
  label === "control-bar" ? <ControlBar /> : label === "picker" ? <PickerOverlay /> : label === "snip" ? <SnipOverlay /> : <App />;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<React.StrictMode>{root}</React.StrictMode>);
