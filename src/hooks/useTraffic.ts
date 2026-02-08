import { useEffect, useRef, useState } from "react";
import { api, TrafficStats } from "../api/tauri";

export interface TrafficData {
  upload: number;
  download: number;
  uploadSpeed: number;
  downloadSpeed: number;
}

export function useTraffic(
  connected: boolean,
  onSpeed?: (downloadSpeed: number) => void,
  intervalMs = 1000
): TrafficData {
  const [data, setData] = useState<TrafficData>({
    upload: 0,
    download: 0,
    uploadSpeed: 0,
    downloadSpeed: 0,
  });
  const prevRef = useRef<TrafficStats | null>(null);
  const timerRef = useRef<ReturnType<typeof setInterval>>();

  useEffect(() => {
    if (!connected) {
      setData({ upload: 0, download: 0, uploadSpeed: 0, downloadSpeed: 0 });
      prevRef.current = null;
      return;
    }

    const poll = async () => {
      try {
        const stats = await api.getTrafficStats();
        const prev = prevRef.current;
        let uploadSpeed = 0;
        let downloadSpeed = 0;

        if (prev !== null) {
          const dt = intervalMs / 1000;
          uploadSpeed = Math.max(0, (stats.upload - prev.upload) / dt);
          downloadSpeed = Math.max(0, (stats.download - prev.download) / dt);
        }

        prevRef.current = stats;
        setData({
          upload: stats.upload,
          download: stats.download,
          uploadSpeed,
          downloadSpeed,
        });

        // Push download speed to global sparkline history
        if (prev !== null && onSpeed) {
          onSpeed(downloadSpeed);
        }
      } catch {
        // ignore — core may not have Clash API ready yet
      }
    };

    // Initial delay to let the core start Clash API
    const initTimer = setTimeout(() => {
      poll();
      timerRef.current = setInterval(poll, intervalMs);
    }, 500);

    return () => {
      clearTimeout(initTimer);
      clearInterval(timerRef.current);
    };
  }, [connected, intervalMs, onSpeed]);

  return data;
}
