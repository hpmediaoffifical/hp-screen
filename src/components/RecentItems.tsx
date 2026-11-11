import { formatElapsed, libraryOpen } from "../api";
import { useLibrary } from "../useLibrary";
import Thumb from "./Thumb";

/** The three newest recordings and screenshots, on the record page. */
export default function RecentItems() {
  const { items, error } = useLibrary(null);
  if (error) return <p className="status err">{error}</p>;
  if (!items) return null;
  if (items.length === 0) return <p className="hint">Video và ảnh mới sẽ hiện ở đây.</p>;
  return (
    <ul className="recent">
      {items.slice(0, 3).map((item) => (
        <li key={item.path}>
          <button className="recent-item" title="Mở" onClick={() => libraryOpen(item.path).catch(() => {})}>
            <Thumb item={item} />
            <span className="recent-name">{item.name}</span>
            <span className="recent-meta">{item.duration_ms !== null ? formatElapsed(item.duration_ms) : "Ảnh"}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}
