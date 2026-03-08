import { useState, useEffect, useMemo } from "react";
import { api, ConnectionRecord, DailyTraffic, ServerUsageStat } from "../../api/tauri";
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
  const [dailyTraffic, setDailyTraffic] = useState<DailyTraffic[]>([]);
  const [serverUsage, setServerUsage] = useState<ServerUsageStat[]>([]);

  useEffect(() => {
    api.getConnectionHistory().then(setHistory).catch(() => {});
    api.getDailyTraffic().then(setDailyTraffic).catch(() => {});
    api.getServerUsageStats().then(setServerUsage).catch(() => {});
  }, []);

  const totalUpload = history.reduce((s, r) => s + r.upload_bytes, 0);
  const totalDownload = history.reduce((s, r) => s + r.download_bytes, 0);
  const totalSessions = history.length;
  const totalTime = history.reduce((s, r) => {
    if (r.disconnected_at) return s + (r.disconnected_at - r.connected_at);
    return s;
  }, 0);

  const speedHistory = state.speedHistory || [];

  // Protocol distribution from history
  const protocolDist = useMemo(() => {
    const map = new Map<string, number>();
    for (const r of history) {
      map.set(r.protocol, (map.get(r.protocol) || 0) + 1);
    }
    const entries = Array.from(map.entries()).sort((a, b) => b[1] - a[1]);
    const total = entries.reduce((s, [, c]) => s + c, 0);
    const colors = ["var(--accent)", "var(--success)", "var(--warning)", "var(--danger)", "var(--text-muted)"];
    return entries.map(([name, count], i) => ({
      name,
      count,
      percent: total > 0 ? (count / total) * 100 : 0,
      color: colors[i % colors.length],
    }));
  }, [history]);

  // Max traffic value for bar chart scaling
  const maxTraffic = useMemo(() => {
    if (dailyTraffic.length === 0) return 1;
    return Math.max(...dailyTraffic.map(d => d.upload + d.download), 1);
  }, [dailyTraffic]);

  // Top servers (max 5)
  const topServers = useMemo(() => {
    return serverUsage.slice(0, 5);
  }, [serverUsage]);

  const maxServerTraffic = useMemo(() => {
    if (topServers.length === 0) return 1;
    return Math.max(...topServers.map(s => s.total_traffic), 1);
  }, [topServers]);

  const renderOverview = () => (
    <>
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
        <div className="stat-card glass-card">
          <div className="stat-value">{totalSessions}</div>
          <div className="stat-label">{t("stats.sessions")}</div>
        </div>
        <div className="stat-card glass-card">
          <div className="stat-value">{formatDuration(totalTime)}</div>
          <div className="stat-label">{t("stats.totalTime")}</div>
        </div>
        <div className="stat-card glass-card">
          <div className="stat-value">{formatBytes(totalUpload)}</div>
          <div className="stat-label">{t("stats.totalUpload")}</div>
        </div>
        <div className="stat-card glass-card">
          <div className="stat-value">{formatBytes(totalDownload)}</div>
          <div className="stat-label">{t("stats.totalDownload")}</div>
        </div>
      </div>
    </>
  );

  const renderTraffic = () => (
    <div className="dashboard-grid">
      <div className="dashboard-card glass-card">
        <div className="stats-label">{t("stats.dailyTraffic")}</div>
        {dailyTraffic.length === 0 ? (
          <div className="empty-list">{t("stats.noHistory")}</div>
        ) : (
          <div className="traffic-chart">
            {dailyTraffic.map((d, i) => {
              const total = d.upload + d.download;
              const height = (total / maxTraffic) * 100;
              const upHeight = total > 0 ? (d.upload / total) * height : 0;
              return (
                <div key={i} className="traffic-bar-col" title={`${d.date}: ${formatBytes(total)}`}>
                  <div className="traffic-bar-wrapper" style={{ height: "80px" }}>
                    <div className="traffic-bar download" style={{ height: `${height}%` }} />
                    <div className="traffic-bar upload" style={{ height: `${upHeight}%` }} />
                  </div>
                  <span className="traffic-bar-label">{d.date.slice(5)}</span>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <div className="dashboard-card glass-card">
        <div className="stats-label">{t("stats.protocols")}</div>
        {protocolDist.length === 0 ? (
          <div className="empty-list">{t("stats.noHistory")}</div>
        ) : (
          <div className="protocol-chart">
            <svg viewBox="0 0 120 120" className="donut-chart">
              {(() => {
                let offset = 0;
                const r = 45;
                const c = 2 * Math.PI * r;
                return protocolDist.map((p, i) => {
                  const dash = (p.percent / 100) * c;
                  const el = (
                    <circle
                      key={i}
                      cx="60" cy="60" r={r}
                      fill="none"
                      stroke={p.color}
                      strokeWidth="12"
                      strokeDasharray={`${dash} ${c - dash}`}
                      strokeDashoffset={-offset}
                      transform="rotate(-90 60 60)"
                    />
                  );
                  offset += dash;
                  return el;
                });
              })()}
              <text x="60" y="56" textAnchor="middle" fill="var(--text-primary)" fontSize="14" fontWeight="700">
                {protocolDist.length}
              </text>
              <text x="60" y="72" textAnchor="middle" fill="var(--text-muted)" fontSize="9">
                protocols
              </text>
            </svg>
            <div className="protocol-legend">
              {protocolDist.map((p, i) => (
                <div key={i} className="protocol-legend-item">
                  <span className="protocol-dot" style={{ background: p.color }} />
                  <span className="protocol-name">{p.name}</span>
                  <span className="protocol-pct">{p.percent.toFixed(0)}%</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );

  const renderHistory = () => (
    <>
      {topServers.length > 0 && (
        <div className="stats-section">
          <div className="stats-label">{t("stats.topServers")}</div>
          <div className="top-servers">
            {topServers.map((s, i) => (
              <div key={i} className="top-server-row">
                <span className="top-server-rank">#{i + 1}</span>
                <span className="top-server-name">{s.server_name}</span>
                <span className="top-server-proto">{s.protocol}</span>
                <div className="top-server-bar-bg">
                  <div
                    className="top-server-bar-fill"
                    style={{ width: `${(s.total_traffic / maxServerTraffic) * 100}%` }}
                  />
                </div>
                <span className="top-server-traffic">{formatBytes(s.total_traffic)}</span>
              </div>
            ))}
          </div>
        </div>
      )}

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
            {[...history].reverse().slice(0, 20).map((r, i) => {
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
    </>
  );

  return (
    <div className="stats-page">
      <div className="stats-header">
        <h2>{t("stats.dashboard")}</h2>
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

      {state.statsTab === "overview" && renderOverview()}
      {state.statsTab === "traffic" && renderTraffic()}
      {state.statsTab === "history" && renderHistory()}
    </div>
  );
}
