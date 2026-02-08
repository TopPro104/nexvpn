import { useEffect, useRef, useCallback } from "react";
import { api, StatusResponse } from "../api/tauri";

export function useStatus(
  onStatus: (status: StatusResponse) => void,
  intervalMs = 3000
) {
  const timerRef = useRef<ReturnType<typeof setInterval>>();

  const poll = useCallback(async () => {
    try {
      const status = await api.getStatus();
      onStatus(status);
    } catch {
      // ignore polling errors
    }
  }, [onStatus]);

  useEffect(() => {
    poll();
    timerRef.current = setInterval(poll, intervalMs);
    return () => clearInterval(timerRef.current);
  }, [poll, intervalMs]);

  return { refresh: poll };
}
