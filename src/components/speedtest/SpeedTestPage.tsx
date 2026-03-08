import { useState } from "react";
import { useApp } from "../../context/AppContext";
import { api, SpeedTestResult } from "../../api/tauri";
import { t } from "../../i18n/translations";
import { GaugeIcon } from "../ui/Icons";

interface TestRecord {
  timestamp: number;
  result: SpeedTestResult;
}

export function SpeedTestPage() {
  const { state, toast } = useApp();
  void state.langTick;
  const [testing, setTesting] = useState(false);
  const [phase, setPhase] = useState<string>("");
  const [result, setResult] = useState<SpeedTestResult | null>(null);
  const [history, setHistory] = useState<TestRecord[]>([]);

  const runTest = async () => {
    if (!state.connected) {
      toast(t("speedtest.connectFirst"), "error");
      return;
    }
    setTesting(true);
    setResult(null);
    setPhase("Ping...");

    try {
      setPhase("Download...");
      const res = await api.runSpeedTest();
      setResult(res);
      setHistory(prev => [{ timestamp: Date.now(), result: res }, ...prev].slice(0, 10));
    } catch (e) {
      toast(`Speed test failed: ${e}`, "error");
    }
    setPhase("");
    setTesting(false);
  };

  // SVG gauge for speed display
  const renderGauge = (value: number, max: number, label: string, unit: string, color: string) => {
    const radius = 60;
    const circumference = Math.PI * radius; // semicircle
    const progress = Math.min(value / max, 1) * circumference;

    return (
      <div className="speed-gauge">
        <svg width="160" height="100" viewBox="0 0 160 100">
          <path
            d="M 20 90 A 60 60 0 0 1 140 90"
            fill="none"
            stroke="var(--border)"
            strokeWidth="8"
            strokeLinecap="round"
          />
          <path
            d="M 20 90 A 60 60 0 0 1 140 90"
            fill="none"
            stroke={color}
            strokeWidth="8"
            strokeLinecap="round"
            strokeDasharray={circumference}
            strokeDashoffset={circumference - progress}
            style={{ transition: "stroke-dashoffset 1s ease" }}
          />
          <text x="80" y="70" textAnchor="middle" fill="var(--text-primary)" fontSize="28" fontWeight="700">
            {value > 0 ? value.toFixed(1) : "—"}
          </text>
          <text x="80" y="90" textAnchor="middle" fill="var(--text-muted)" fontSize="11">
            {unit}
          </text>
        </svg>
        <div className="speed-gauge-label">{label}</div>
      </div>
    );
  };

  const formatDate = (ts: number) => {
    const d = new Date(ts);
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  };

  return (
    <div className="speedtest-page">
      <div className="speedtest-header">
        <h2><GaugeIcon size={24} /> {t("speedtest.title")}</h2>
      </div>

      <div className="speedtest-gauges">
        {renderGauge(
          result?.download_mbps || 0, 200,
          t("speedtest.download"), t("speedtest.mbps"), "var(--success)"
        )}
        {renderGauge(
          result?.upload_mbps || 0, 100,
          t("speedtest.upload"), t("speedtest.mbps"), "var(--accent)"
        )}
        <div className="speed-ping-card">
          <div className="speed-ping-value">
            {result ? result.ping_ms : "—"}
          </div>
          <div className="speed-ping-unit">{t("speedtest.ms")}</div>
          <div className="speed-ping-label">{t("speedtest.ping")}</div>
        </div>
      </div>

      <div className="speedtest-actions">
        <button
          className={`btn btn-md ${testing ? "btn-secondary" : "btn-primary"} speedtest-btn`}
          onClick={runTest}
          disabled={testing}
        >
          {testing ? (
            <><span className="speedtest-spinner" /> {phase || t("speedtest.testing")}</>
          ) : (
            t("speedtest.start")
          )}
        </button>
      </div>

      {!state.connected && (
        <div className="speedtest-hint">{t("speedtest.connectFirst")}</div>
      )}

      {history.length > 0 && (
        <div className="speedtest-history">
          <div className="stats-label">{t("speedtest.history")}</div>
          <div className="history-table">
            <div className="history-header" style={{ gridTemplateColumns: "1fr 1fr 1fr 1fr" }}>
              <span>{t("stats.date")}</span>
              <span>{t("speedtest.download")}</span>
              <span>{t("speedtest.upload")}</span>
              <span>{t("speedtest.ping")}</span>
            </div>
            {history.map((h, i) => (
              <div key={i} className="history-row" style={{ gridTemplateColumns: "1fr 1fr 1fr 1fr" }}>
                <span className="history-date">{formatDate(h.timestamp)}</span>
                <span>{h.result.download_mbps.toFixed(1)} Mbps</span>
                <span>{h.result.upload_mbps.toFixed(1)} Mbps</span>
                <span>{h.result.ping_ms} ms</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
