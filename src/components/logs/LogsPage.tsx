import { useState, useEffect, useRef } from "react";
import { api } from "../../api/tauri";
import { useApp } from "../../context/AppContext";
import { Button } from "../ui/Button";
import { t } from "../../i18n/translations";

export function LogsPage() {
  const { state } = useApp();
  void state.langTick;
  const [logs, setLogs] = useState<string[]>([]);
  const [autoScroll, setAutoScroll] = useState(true);
  const [filter, setFilter] = useState("");
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const poll = async () => {
      try {
        const data = await api.getLogs();
        setLogs(data);
      } catch {
        // ignore
      }
    };
    poll();
    const id = setInterval(poll, 1500);
    return () => clearInterval(id);
  }, []);

  useEffect(() => {
    if (autoScroll && containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [logs, autoScroll]);

  const filtered = filter
    ? logs.filter((l) => l.toLowerCase().includes(filter.toLowerCase()))
    : logs;

  const handleClear = async () => {
    await api.clearLogs();
    setLogs([]);
  };

  const handleCopy = () => {
    navigator.clipboard.writeText(filtered.join("\n"));
  };

  const getLineClass = (line: string) => {
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
          <div className="logs-empty">{t("logs.empty")}</div>
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
