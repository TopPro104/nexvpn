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
        <div className="traffic-icon upload-icon">&uarr;</div>
        <div className="traffic-data">
          <div className="traffic-speed">{formatSpeed(traffic.uploadSpeed)}</div>
          <div className="traffic-total">{formatBytes(traffic.upload)}</div>
        </div>
      </div>
      <div className="traffic-sparkline">
        <Sparkline data={state.speedHistory} width={100} height={28} />
      </div>
      <div className="traffic-item">
        <div className="traffic-icon download-icon">&darr;</div>
        <div className="traffic-data">
          <div className="traffic-speed">{formatSpeed(traffic.downloadSpeed)}</div>
          <div className="traffic-total">{formatBytes(traffic.download)}</div>
        </div>
      </div>
    </div>
  );
}
