import { useState, useEffect, useRef } from "react";
import { api } from "../../api/tauri";
import { useApp } from "../../context/AppContext";
import { Button } from "../ui/Button";
import { t } from "../../i18n/translations";

type LogTab = "core" | "app";

export function LogsPage() {
  const { state } = useApp();
  void state.langTick;
  const [coreLogs, setCoreLogs] = useState<string[]>([]);
  const [appLogs, setAppLogs] = useState<string[]>([]);
  const [logTab, setLogTab] = useState<LogTab>("core");
  const [autoScroll, setAutoScroll] = useState(true);
  const [filter, setFilter] = useState("");
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const poll = async () => {
      try {
        const [core, app] = await Promise.all([
          api.getLogs(),
          api.getAppLogs(),
        ]);
        setCoreLogs(core);
        setAppLogs(app);
      } catch {
        // ignore
      }
    };
    poll();
    const id = setInterval(poll, 1500);
    return () => clearInterval(id);
  }, []);

  const logs = logTab === "core" ? coreLogs : appLogs;

  useEffect(() => {
    if (autoScroll && containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [logs, autoScroll]);

  // Apply top-bar level filter
  let levelFiltered = logs;
  if (state.logsFilter === "errors") {
    levelFiltered = logs.filter((l) => {
      const lower = l.toLowerCase();
      return lower.includes("error") || lower.includes("fatal") || l.startsWith("✗");
    });
  } else if (state.logsFilter === "warnings") {
    levelFiltered = logs.filter((l) => l.toLowerCase().includes("warn"));
  }

  const filtered = filter
    ? levelFiltered.filter((l) => l.toLowerCase().includes(filter.toLowerCase()))
    : levelFiltered;

  const handleClear = async () => {
    if (logTab === "core") {
      await api.clearLogs();
      setCoreLogs([]);
    } else {
      await api.clearAppLogs();
      setAppLogs([]);
    }
  };

  const handleCopy = () => {
    navigator.clipboard.writeText(filtered.join("\n"));
  };

  const getLineClass = (line: string) => {
    if (line.startsWith("✗")) return "log-error";
    if (line.startsWith("✓")) return "log-info";
    const lower = line.toLowerCase();
    if (lower.includes("error") || lower.includes("fatal")) return "log-error";
    if (lower.includes("warn")) return "log-warn";
    if (lower.includes("info")) return "log-info";
    return "";
  };

  return (
    <div className="logs-page">
      <div className="logs-header">
        <h2>{t("nav.logs")}</h2>
        <div className="logs-actions">
          <div className="logs-tabs">
            <button
              className={`logs-tab${logTab === "core" ? " active" : ""}`}
              onClick={() => setLogTab("core")}
            >
              {t("logs.core")}
            </button>
            <button
              className={`logs-tab${logTab === "app" ? " active" : ""}`}
              onClick={() => setLogTab("app")}
            >
              {t("logs.app")}
              {appLogs.length > 0 && <span className="logs-tab-badge">{appLogs.length}</span>}
            </button>
          </div>
          <input
            className="search-input logs-filter"
            placeholder={t("logs.filter")}
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
          <label className="logs-autoscroll">
            <input
              type="checkbox"
              checked={autoScroll}
              onChange={(e) => setAutoScroll(e.target.checked)}
            />
            <span>{t("logs.autoScroll")}</span>
          </label>
          <Button variant="ghost" size="sm" onClick={handleCopy}>
            {t("logs.copy")}
          </Button>
          <Button variant="ghost" size="sm" onClick={handleClear}>
            {t("logs.clear")}
          </Button>
        </div>
      </div>
      <div className="logs-container" ref={containerRef}>
        {filtered.length === 0 ? (
          <div className="logs-empty">
            {logTab === "core" ? t("logs.empty") : t("logs.appEmpty")}
          </div>
        ) : (
          filtered.map((line, i) => (
            <div key={i} className={`log-line ${getLineClass(line)}`}>
              {line}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
