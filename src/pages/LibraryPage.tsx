import { useEffect, useRef, useState } from "react";
import {
  errorMessage,
  formatElapsed,
  formatSize,
  libraryDelete,
  libraryOpen,
  libraryRename,
  libraryReveal,
  type LibraryEntry,
  type MediaKind,
} from "../api";
import Thumb from "../components/Thumb";
import { useLibrary } from "../useLibrary";

const FILTERS: { value: MediaKind | null; label: string }[] = [
  { value: null, label: "Tất cả" },
  { value: "video", label: "Video" },
  { value: "image", label: "Ảnh" },
];

const stemOf = (name: string) => name.replace(/\.[^.]+$/, "");

export default function LibraryPage(props: { onEdit?: (path: string) => void } = {}) {
  const [filter, setFilter] = useState<MediaKind | null>(null);
  const { items, error } = useLibrary(filter);
  const [selected, setSelected] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<LibraryEntry | null>(null);
  const [message, setMessage] = useState("");

  const current = items?.find((i) => i.path === selected) ?? null;

  async function run(action: () => Promise<unknown>) {
    try {
      await action();
      setMessage("");
    } catch (e) {
      setMessage(errorMessage(e));
    }
  }

  // F2 renames and Delete removes the selected item, as in File Explorer.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!current || renaming || confirming) return;
      if (e.key === "F2") {
        e.preventDefault();
        setRenaming(current.path);
      } else if (e.key === "Delete") {
        e.preventDefault();
        setConfirming(current);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [current, renaming, confirming]);

  return (
    <section className="page library">
      <div className="page-head">
        <h1>Thư viện</h1>
        <div className="row" role="group" aria-label="Lọc theo loại">
          {FILTERS.map((f) => (
            <button key={f.label} className={"chip chip-toggle" + (filter === f.value ? " on" : "")} aria-pressed={filter === f.value} onClick={() => setFilter(f.value)}>
              {f.label}
            </button>
          ))}
          <button className="btn btn-small" onClick={() => run(() => libraryReveal(null))}>
            Mở thư mục lưu
          </button>
        </div>
      </div>

      {error && <p className="status err" role="alert">{error}</p>}
      {message && <p className="status err" role="alert">{message}</p>}

      {items && items.length === 0 && (
        <div className="empty">
          <p className="empty-title">Chưa có gì ở đây</p>
          <p className="hint">Video bạn quay và ảnh bạn chụp sẽ xuất hiện trong thư viện, mới nhất ở trên cùng.</p>
        </div>
      )}

      <ul className="grid">
        {items?.map((item) => (
          <li key={item.path}>
            <div
              className={"card" + (selected === item.path ? " selected" : "")}
              tabIndex={0}
              onClick={() => setSelected(item.path)}
              onFocus={() => setSelected(item.path)}
              onDoubleClick={() => run(() => libraryOpen(item.path))}
              onKeyDown={(e) => {
                if (e.key === "Enter" && renaming !== item.path) run(() => libraryOpen(item.path));
              }}
            >
              <Thumb item={item} />
              {renaming === item.path ? (
                <RenameField
                  initial={stemOf(item.name)}
                  onCancel={() => setRenaming(null)}
                  onSubmit={(stem) => {
                    setRenaming(null);
                    run(async () => setSelected(await libraryRename(item.path, stem)));
                  }}
                />
              ) : (
                <span className="card-name" title={item.name}>{item.name}</span>
              )}
              <span className="card-meta">
                {[
                  item.duration_ms !== null ? formatElapsed(item.duration_ms) : null,
                  item.width && item.height ? `${item.width}×${item.height}` : null,
                  formatSize(item.size),
                ]
                  .filter(Boolean)
                  .join(" · ")}
              </span>
              <div className="card-actions">
                <button className="btn btn-small" onClick={() => run(() => libraryOpen(item.path))}>
                  {item.kind === "video" ? "Phát" : "Mở"}
                </button>
                {item.kind === "video" && props.onEdit && (
                  <button className="btn btn-small" onClick={() => props.onEdit?.(item.path)}>Chỉnh sửa</button>
                )}
                <button className="btn btn-small" onClick={() => run(() => libraryReveal(item.path))}>Thư mục</button>
                <button className="btn btn-small" onClick={() => setRenaming(item.path)}>Đổi tên</button>
                <button className="btn btn-small btn-danger" onClick={() => setConfirming(item)}>Xóa</button>
              </div>
            </div>
          </li>
        ))}
      </ul>

      {confirming && (
        <ConfirmDelete
          item={confirming}
          onCancel={() => setConfirming(null)}
          onConfirm={() => {
            const target = confirming;
            setConfirming(null);
            setSelected(null);
            run(() => libraryDelete(target.path));
          }}
        />
      )}
    </section>
  );
}

function RenameField(props: { initial: string; onSubmit: (stem: string) => void; onCancel: () => void }) {
  const [value, setValue] = useState(props.initial);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.select(), []);
  return (
    <input
      ref={input}
      className="rename"
      aria-label="Tên mới"
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onClick={(e) => e.stopPropagation()}
      onBlur={props.onCancel}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter" && value.trim()) props.onSubmit(value);
        if (e.key === "Escape") props.onCancel();
      }}
    />
  );
}

function ConfirmDelete(props: { item: LibraryEntry; onConfirm: () => void; onCancel: () => void }) {
  const cancel = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    cancel.current?.focus();
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && props.onCancel();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [props]);
  return (
    <div className="modal-back" onClick={props.onCancel}>
      <div className="modal" role="alertdialog" aria-modal="true" aria-labelledby="del-title" onClick={(e) => e.stopPropagation()}>
        <h2 id="del-title">Xóa “{props.item.name}”?</h2>
        <p className="hint">File sẽ được chuyển vào Thùng rác của Windows, bạn có thể khôi phục từ đó.</p>
        <div className="row modal-actions">
          <button ref={cancel} className="btn" onClick={props.onCancel}>Giữ lại</button>
          <button className="btn btn-rec" onClick={props.onConfirm}>Xóa</button>
        </div>
      </div>
    </div>
  );
}
