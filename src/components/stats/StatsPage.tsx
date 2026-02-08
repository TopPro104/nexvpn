import { useState, useEffect } from "react";
import { api, ConnectionRecord } from "../../api/tauri";
import { useApp } from "../../context/AppContext";
import { Sparkline } from "../ui/Sparkline";
import { Button } from "../ui/Button";
import { t } from "../../i18n/translations";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return (bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0) + " " + units[i];
}

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return `${h}h ${m}m`;
}

function formatDate(ts: number): string {
  return new Date(ts * 1000).toLocaleString();
}

export function StatsPage() {
  const { state, toast } = useApp();
  void state.langTick;
  const [history, setHistory] = useState<ConnectionRecord[]>([]);

  useEffect(() => {
    api.getConnectionHistory().then(setHistory).catch(() => {});
  }, []);

  const totalUpload = history.reduce((s, r) => s + r.upload_bytes, 0);
  const totalDownload = history.reduce((s, r) => s + r.download_bytes, 0);
  const totalSessions = history.length;
  const totalTime = history.reduce((s, r) => {
    if (r.disconnected_at) return s + (r.disconnected_at - r.connected_at);
    return s;
  }, 0);

  // Speed history from global state for sparkline
  const speedHistory = state.speedHistory || [];

  return (
    <div className="stats-page">
      <div className="stats-header">
        <h2>{t("nav.stats")}</h2>
        {history.length > 0 && (
          <Button
            variant="danger"
            size="sm"
            onClick={async () => {
              if (!window.confirm(t("stats.resetConfirm"))) return;
              try {
                await api.clearConnectionHistory();
                setHistory([]);
                toast(t("toast.statsReset"), "success");
              } catch (e) {
                toast(`${e}`, "error");
              }
            }}
          >
            {t("stats.reset")}
          </Button>
        )}
      </div>

      {/* Speed graph */}
      <div className="stats-section">
        <div className="stats-label">{t("stats.speedGraph")}</div>
        <div className="stats-sparkline-container">
          <Sparkline data={speedHistory} width={400} height={60} color="var(--accent)" />
          {speedHistory.length < 2 && (
            <div className="stats-sparkline-hint">{t("stats.connectToSee")}</div>
          )}
        </div>
      </div>

      {/* Summary cards */}
      <div className="stats-summary">
        <div className="stat-card">
          <div className="stat-value">{totalSessions}</div>
          <div className="stat-label">{t("stats.sessions")}</div>
        </div>
        <div className="stat-card">
          <div className="stat-value">{formatDuration(totalTime)}</div>
          <div className="stat-label">{t("stats.totalTime")}</div>
        </div>
        <div className="stat-card">
          <div className="stat-value">{formatBytes(totalUpload)}</div>
          <div className="stat-label">{t("stats.totalUpload")}</div>
        </div>
        <div className="stat-card">
          <div className="stat-value">{formatBytes(totalDownload)}</div>
          <div className="stat-label">{t("stats.totalDownload")}</div>
        </div>
      </div>

      {/* History table */}
      <div className="stats-section">
        <div className="stats-label">{t("stats.history")}</div>
        {history.length === 0 ? (
          <div className="empty-list">{t("stats.noHistory")}</div>
        ) : (
          <div className="history-table">
            <div className="history-header">
              <span>{t("stats.server")}</span>
              <span>{t("stats.duration")}</span>
              <span>{t("stats.traffic")}</span>
              <span>{t("stats.date")}</span>
            </div>
            {[...history].reverse().map((r, i) => {
              const dur = r.disconnected_at
                ? r.disconnected_at - r.connected_at
                : 0;
              return (
                <div key={i} className="history-row">
                  <span className="history-server">
                    <span className="history-proto">{r.protocol}</span>
                    {r.server_name}
                  </span>
                  <span>{dur > 0 ? formatDuration(dur) : "--"}</span>
                  <span>
                    {formatBytes(r.upload_bytes)} / {formatBytes(r.download_bytes)}
                  </span>
                  <span className="history-date">{formatDate(r.connected_at)}</span>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
