import { convertFileSrc } from "@tauri-apps/api/core";
import { useState } from "react";
import type { LibraryEntry } from "../api";

/** Preview of a library item; falls back to a plain tile when there is no image to show. */
export default function Thumb({ item }: { item: LibraryEntry }) {
  const [failed, setFailed] = useState(false);
  // Screenshots without a saved preview can show the picture itself.
  const source = item.thumb ?? (item.kind === "image" ? item.path : null);
  if (!source || failed) {
    return <div className="thumb thumb-empty" aria-hidden="true">{item.kind === "video" ? "▶" : "▣"}</div>;
  }
  return <img className="thumb" src={convertFileSrc(source)} alt="" loading="lazy" onError={() => setFailed(true)} />;
}
