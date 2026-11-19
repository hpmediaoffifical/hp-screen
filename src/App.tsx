import { useState } from "react";
import { compactShow } from "./api";
import RecordPage from "./pages/RecordPage";
import LibraryPage from "./pages/LibraryPage";
import SettingsPage from "./pages/SettingsPage";
import EditPage from "./pages/EditPage";
import UpdateNotice from "./components/UpdateNotice";
import HotkeyWarning, { type HotkeyField } from "./components/HotkeyWarning";

type PageId = "record" | "library" | "settings";

const NAV: { id: PageId; label: string; icon: string }[] = [
  { id: "record", label: "Quay", icon: "●" },
  { id: "library", label: "Thư viện", icon: "▤" },
  { id: "settings", label: "Cài đặt", icon: "⚙" },
];

export default function App() {
  const [page, setPage] = useState<PageId>("record");
  // A video open in the edit page replaces the current page until it is closed.
  const [editing, setEditing] = useState<{ path: string; seekMs: number | null } | null>(null);
  /** Hotkey field to bring into view when Settings opens from the hotkey warning. */
  const [focusHotkey, setFocusHotkey] = useState<HotkeyField | null>(null);
  return (
    <div className="app">
      <nav className="sidebar" aria-label="Điều hướng chính">
        <div className="brand">HP Screen</div>
        {NAV.map((n) => (
          <button
            key={n.id}
            className={"nav-item" + (page === n.id ? " active" : "")}
            aria-current={page === n.id ? "page" : undefined}
            onClick={() => {
              setEditing(null);
              setFocusHotkey(null);
              setPage(n.id);
            }}
          >
            <span className="nav-icon" aria-hidden="true">{n.icon}</span>
            {n.label}
          </button>
        ))}
        <button className="nav-item sidebar-bottom" title="Thu HP Screen thành một thanh nhỏ nổi trên màn hình" onClick={() => compactShow().catch(() => {})}>
          <span className="nav-icon" aria-hidden="true">▭</span>
          Thanh gọn
        </button>
      </nav>
      <main className="content">
        {editing && <EditPage key={editing.path} path={editing.path} seekMs={editing.seekMs} onClose={() => setEditing(null)} />}
        {!editing && (
          <>
            <HotkeyWarning
              onOpenSettings={(field) => {
                setFocusHotkey(field);
                setPage("settings");
              }}
            />
            {page === "record" && <RecordPage />}
            {page === "library" && (
              <LibraryPage
                onEdit={(path, seekMs) => {
                  setEditing({ path, seekMs: seekMs ?? null });
                  setFocusHotkey(null);
                }}
              />
            )}
            {page === "settings" && <SettingsPage focusHotkey={focusHotkey} />}
          </>
        )}
      </main>
      <UpdateNotice />
    </div>
  );
}
