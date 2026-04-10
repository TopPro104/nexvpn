import { useCallback } from "react";
import { useApp } from "../../context/AppContext";
import { useTraffic } from "../../hooks/useTraffic";
import { Sparkline } from "../ui/Sparkline";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return (bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0) + " " + units[i];
}

function formatSpeed(bytesPerSec: number): string {
  return formatBytes(bytesPerSec) + "/s";
}

export function TrafficPanel() {
  const { state, dispatch } = useApp();

  const onSpeed = useCallback(
    (speed: number) => dispatch({ type: "PUSH_SPEED", speed }),
    [dispatch]
  );

  const traffic = useTraffic(state.connected, onSpeed);

  return (
    <div className="traffic-panel">
      <div className="traffic-item">
        <div className="traffic-icon upload-icon">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M8 13V3M8 3L3.5 7.5M8 3L12.5 7.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
        </div>
        <div className="traffic-data">
          <div className="traffic-speed">{formatSpeed(traffic.uploadSpeed)}</div>
          <div className="traffic-total">{formatBytes(traffic.upload)}</div>
        </div>
      </div>
      <div className="traffic-sparkline">
        <Sparkline data={state.speedHistory} width={100} height={28} />
      </div>
      <div className="traffic-item">
        <div className="traffic-icon download-icon">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M8 3V13M8 13L3.5 8.5M8 13L12.5 8.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
        </div>
        <div className="traffic-data">
          <div className="traffic-speed">{formatSpeed(traffic.downloadSpeed)}</div>
          <div className="traffic-total">{formatBytes(traffic.download)}</div>
        </div>
      </div>
    </div>
  );
}
