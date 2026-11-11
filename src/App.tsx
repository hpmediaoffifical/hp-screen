import { useState } from "react";
import RecordPage from "./pages/RecordPage";
import LibraryPage from "./pages/LibraryPage";
import SettingsPage from "./pages/SettingsPage";

type PageId = "record" | "library" | "settings";

const NAV: { id: PageId; label: string; icon: string }[] = [
  { id: "record", label: "Quay", icon: "●" },
  { id: "library", label: "Thư viện", icon: "▤" },
  { id: "settings", label: "Cài đặt", icon: "⚙" },
];

export default function App() {
  const [page, setPage] = useState<PageId>("record");
  return (
    <div className="app">
      <nav className="sidebar" aria-label="Điều hướng chính">
        <div className="brand">HP Screen</div>
        {NAV.map((n) => (
          <button
            key={n.id}
            className={"nav-item" + (page === n.id ? " active" : "")}
            aria-current={page === n.id ? "page" : undefined}
            onClick={() => setPage(n.id)}
          >
            <span className="nav-icon" aria-hidden="true">{n.icon}</span>
            {n.label}
          </button>
        ))}
      </nav>
      <main className="content">
        {page === "record" && <RecordPage />}
        {page === "library" && <LibraryPage />}
        {page === "settings" && <SettingsPage />}
      </main>
    </div>
  );
}
