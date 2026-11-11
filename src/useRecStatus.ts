import { useEffect, useState } from "react";
import { onRecError, onRecState, recStatus, type RecStatus } from "./api";

const IDLE: RecStatus = { state: "idle", elapsed_ms: 0, countdown: null, file: null, warning: null };

/** Live recording status plus the most recent error message from the core. */
export function useRecStatus(): { status: RecStatus; error: string; clearError: () => void } {
  const [status, setStatus] = useState<RecStatus>(IDLE);
  const [error, setError] = useState("");

  useEffect(() => {
    let active = true;
    recStatus().then((s) => active && setStatus(s)).catch(() => {});
    const unlisten = [onRecState((s) => active && setStatus(s)), onRecError((m) => active && setError(m))];
    return () => {
      active = false;
      unlisten.forEach((p) => p.then((off) => off()));
    };
  }, []);

  return { status, error, clearError: () => setError("") };
}
