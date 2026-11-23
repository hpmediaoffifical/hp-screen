/*! © 2026 HP Media (hpvn.media) — HP Screen. Bảo lưu mọi quyền. HP không có đại lý khác, không hợp tác với đơn vị nào, mọi sao chép đều là vi phạm. */
import { useCallback, useEffect, useState } from "react";
import { errorMessage, libraryList, onLibraryChanged, type LibraryEntry, type MediaKind } from "./api";

/** The library listing, refreshed whenever the core reports a change. */
export function useLibrary(filter: MediaKind | null) {
  const [items, setItems] = useState<LibraryEntry[] | null>(null);
  const [error, setError] = useState("");

  const reload = useCallback(() => {
    libraryList(filter)
      .then((list) => {
        setItems(list);
        setError("");
      })
      .catch((e) => setError(errorMessage(e)));
  }, [filter]);

  useEffect(() => {
    reload();
    const unlisten = onLibraryChanged(reload);
    return () => {
      unlisten.then((off) => off());
    };
  }, [reload]);

  return { items, error, reload };
}
