import { useSyncExternalStore } from "react";
import {
  errorMessage,
  onUpdateAvailable,
  onUpdateProgress,
  updateCheck,
  updateInstall,
  updatePending,
  type UpdateInfo,
  type UpdateProgress,
} from "./api";

/** One copy shared by the update notice and the settings page. */
export interface UpdateView {
  available: UpdateInfo | null;
  /** The version the user chose "Để sau" for; its notice stays hidden until the next start. */
  dismissed: string | null;
  checking: boolean;
  progress: UpdateProgress | null;
  /** Result of the last "Kiểm tra cập nhật" or install attempt. */
  note: { text: string; error: boolean } | null;
}

let view: UpdateView = { available: null, dismissed: null, checking: false, progress: null, note: null };
const listeners = new Set<() => void>();
let started = false;

function set(patch: Partial<UpdateView>) {
  view = { ...view, ...patch };
  listeners.forEach((l) => l());
}

function start() {
  if (started) return;
  started = true;
  updatePending().then((u) => u && set({ available: u })).catch(() => {});
  onUpdateAvailable((u) => set({ available: u }));
  onUpdateProgress((p) => set(p.phase === "failed" ? { progress: null, note: { text: p.message, error: true } } : { progress: p, note: null }));
}

function subscribe(listener: () => void) {
  start();
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export async function checkForUpdate() {
  set({ checking: true, note: null });
  try {
    const found = await updateCheck();
    // Asking by hand shows the notice again even for a version put off earlier.
    set({ available: found, dismissed: null, note: found ? null : { text: "HP Screen đã là bản mới nhất.", error: false } });
  } catch (e) {
    set({ note: { text: errorMessage(e), error: true } });
  } finally {
    set({ checking: false });
  }
}

export async function installUpdate() {
  set({ note: null, progress: { phase: "downloading", downloaded: 0, total: null } });
  try {
    await updateInstall();
  } catch (e) {
    set({ progress: null, note: { text: errorMessage(e), error: true } });
  }
}

export function dismissUpdate() {
  if (view.available) set({ dismissed: view.available.version });
}

export function useUpdate(): UpdateView {
  return useSyncExternalStore(subscribe, () => view);
}

/** "Đang tải 42%" and the like, for a progress line. */
export function progressText(p: UpdateProgress): string {
  switch (p.phase) {
    case "downloading":
      return p.total ? `Đang tải bản cập nhật… ${Math.min(100, Math.round((p.downloaded / p.total) * 100))}%` : "Đang tải bản cập nhật…";
    case "waiting_for_recording":
      return "Đã tải xong. Bản mới sẽ được cài ngay khi bạn quay xong.";
    case "installing":
      return "Đang cài bản mới. HP Screen sẽ tự mở lại sau ít giây.";
    case "failed":
      return p.message;
  }
}
