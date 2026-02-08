import { useState } from "react";
import { useApp } from "../../context/AppContext";
import { api } from "../../api/tauri";
import { ServerCard } from "./ServerCard";
import { Button } from "../ui/Button";
import { Spinner } from "../ui/Spinner";
import { t } from "../../i18n/translations";

type SortKey = "name" | "ping" | "protocol";

export function ServerList() {
  const { state, dispatch, toast } = useApp();
  void state.langTick;
  const [search, setSearch] = useState("");
  const [tab, setTab] = useState<string>("all");
  const [sortBy, setSortBy] = useState<SortKey>("name");
  const [pinging, setPinging] = useState(false);
  const [autoSelecting, setAutoSelecting] = useState(false);
  const [showImport, setShowImport] = useState(false);
  const [importText, setImportText] = useState("");
  const [importLoading, setImportLoading] = useState(false);

  const filtered = state.servers.filter((s) => {
    const matchSearch =
      !search ||
      s.name.toLowerCase().includes(search.toLowerCase()) ||
      s.address.toLowerCase().includes(search.toLowerCase()) ||
      s.protocol.toLowerCase().includes(search.toLowerCase());
    const matchTab =
      tab === "all" || s.subscription_id === tab || (!s.subscription_id && tab === "manual");
    return matchSearch && matchTab;
  });

  const sorted = [...filtered].sort((a, b) => {
    if (sortBy === "ping") {
      const ap = a.latency_ms ?? 99999;
      const bp = b.latency_ms ?? 99999;
      return ap - bp;
    }
    if (sortBy === "protocol") return a.protocol.localeCompare(b.protocol);
    return a.name.localeCompare(b.name);
  });

  const tabs: { id: string; label: string }[] = [
    { id: "all", label: `${t("servers.all")} (${state.servers.length})` },
    { id: "manual", label: t("servers.manual") },
    ...state.subscriptions.map((sub) => ({
      id: sub.id,
      label: sub.name,
    })),
  ];

  const handlePingAll = async () => {
    setPinging(true);
    try {
      const results = await api.pingAllServers();
      dispatch({ type: "UPDATE_LATENCIES", results });
      const reachable = results.filter(([, ms]) => ms !== null).length;
      toast(`${t("toast.pingDone")}: ${reachable}/${results.length} ${t("toast.reachable")}`, "success");
    } catch (e) {
      toast(`${e}`, "error");
    }
    setPinging(false);
  };

  const handleAutoSelect = async () => {
    setAutoSelecting(true);
    try {
      const best = await api.autoSelectServer();
      dispatch({ type: "SELECT_SERVER", id: best.id });
      const servers = await api.getServers();
      dispatch({ type: "SET_SERVERS", servers });
      toast(`${t("servers.bestServer")}: ${best.name} (${best.latency_ms}ms)`, "success");
    } catch (e) {
      toast(`${e}`, "error");
    }
    setAutoSelecting(false);
  };

  const handleImport = async () => {
    if (!importText.trim()) return;
    setImportLoading(true);
    try {
      if (importText.trim().startsWith("http")) {
        const servers = await api.addSubscription(importText.trim());
        dispatch({ type: "ADD_SERVERS", servers });
        const subs = await api.getSubscriptions();
        dispatch({ type: "SET_SUBSCRIPTIONS", subs });
        toast(`${t("toast.subAdded")}: ${servers.length}`, "success");
      } else {
        const servers = await api.addLinks(importText.trim());
        dispatch({ type: "ADD_SERVERS", servers });
        toast(`${t("toast.imported")} ${servers.length}`, "success");
      }
      setImportText("");
      setShowImport(false);
    } catch (e) {
      toast(`${e}`, "error");
    }
    setImportLoading(false);
  };

  return (
    <div className="server-list-panel">
      <div className="server-list-header">
        <input
          className="search-input"
          placeholder={t("servers.search")}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <div className="server-list-actions">
          <select
            className="sort-select"
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value as SortKey)}
          >
            <option value="name">{t("servers.sortName")}</option>
            <option value="ping">{t("servers.sortPing")}</option>
            <option value="protocol">{t("servers.sortProto")}</option>
          </select>
          <Button variant="secondary" size="sm" onClick={() => setShowImport(!showImport)}>
            {t("servers.import")}
          </Button>
          <Button variant="secondary" size="sm" onClick={handlePingAll} disabled={pinging}>
            {pinging ? <Spinner size={14} /> : t("servers.pingAll")}
          </Button>
          <Button variant="secondary" size="sm" onClick={handleAutoSelect} disabled={autoSelecting}>
            {autoSelecting ? <Spinner size={14} /> : t("servers.autoSelect")}
          </Button>
        </div>
      </div>

      {showImport && (
        <div className="import-box">
          <textarea
            className="import-textarea"
            placeholder={t("servers.importPlaceholder")}
            value={importText}
            onChange={(e) => setImportText(e.target.value)}
            rows={3}
          />
          <div className="import-actions">
            <Button size="sm" onClick={handleImport} disabled={importLoading}>
              {importLoading ? <Spinner size={14} /> : t("common.import")}
            </Button>
            <Button variant="ghost" size="sm" onClick={() => setShowImport(false)}>
              {t("common.cancel")}
            </Button>
          </div>
        </div>
      )}

      <div className="server-tabs">
        {tabs.map((tt) => (
          <button
            key={tt.id}
            className={`tab-btn ${tab === tt.id ? "active" : ""}`}
            onClick={() => setTab(tt.id)}
          >
            {tt.label}
          </button>
        ))}
      </div>

      <div className="server-list-items">
        {filtered.length === 0 ? (
          <div className="empty-list">
            {state.servers.length === 0 ? t("servers.empty") : t("servers.noMatch")}
          </div>
        ) : (
          sorted.map((s) => <ServerCard key={s.id} server={s} />)
        )}
      </div>
    </div>
  );
}
