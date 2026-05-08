import { useState, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import { useApp, SortKey, ViewMode } from "../../context/AppContext";
import { api, ServerInfo } from "../../api/tauri";
import { ServerCard } from "./ServerCard";
import { Button } from "../ui/Button";
import { Spinner } from "../ui/Spinner";
import { t } from "../../i18n/translations";
import { extractCountryCode } from "../../utils/countryUtils";
import { GridIcon, ListIcon } from "../ui/Icons";

const COUNTRY_FLAGS: Record<string, string> = {
  us: "\u{1F1FA}\u{1F1F8}", gb: "\u{1F1EC}\u{1F1E7}", de: "\u{1F1E9}\u{1F1EA}",
  nl: "\u{1F1F3}\u{1F1F1}", fr: "\u{1F1EB}\u{1F1F7}", jp: "\u{1F1EF}\u{1F1F5}",
  sg: "\u{1F1F8}\u{1F1EC}", ru: "\u{1F1F7}\u{1F1FA}", tr: "\u{1F1F9}\u{1F1F7}",
  ca: "\u{1F1E8}\u{1F1E6}", au: "\u{1F1E6}\u{1F1FA}", hk: "\u{1F1ED}\u{1F1F0}",
  kr: "\u{1F1F0}\u{1F1F7}", in: "\u{1F1EE}\u{1F1F3}", br: "\u{1F1E7}\u{1F1F7}",
  it: "\u{1F1EE}\u{1F1F9}", es: "\u{1F1EA}\u{1F1F8}", se: "\u{1F1F8}\u{1F1EA}",
  ch: "\u{1F1E8}\u{1F1ED}", fi: "\u{1F1EB}\u{1F1EE}", no: "\u{1F1F3}\u{1F1F4}",
  pl: "\u{1F1F5}\u{1F1F1}", ua: "\u{1F1FA}\u{1F1E6}", il: "\u{1F1EE}\u{1F1F1}",
  tw: "\u{1F1F9}\u{1F1FC}", ie: "\u{1F1EE}\u{1F1EA}", at: "\u{1F1E6}\u{1F1F9}",
  be: "\u{1F1E7}\u{1F1EA}", cz: "\u{1F1E8}\u{1F1FF}", dk: "\u{1F1E9}\u{1F1F0}",
  ro: "\u{1F1F7}\u{1F1F4}", bg: "\u{1F1E7}\u{1F1EC}", hu: "\u{1F1ED}\u{1F1FA}",
  pt: "\u{1F1F5}\u{1F1F9}", ae: "\u{1F1E6}\u{1F1EA}", za: "\u{1F1FF}\u{1F1E6}",
  mx: "\u{1F1F2}\u{1F1FD}", ar: "\u{1F1E6}\u{1F1F7}", kz: "\u{1F1F0}\u{1F1FF}",
};

const COUNTRY_NAMES: Record<string, string> = {
  us: "United States", gb: "United Kingdom", de: "Germany", nl: "Netherlands",
  fr: "France", jp: "Japan", sg: "Singapore", ru: "Russia", tr: "Turkey",
  ca: "Canada", au: "Australia", hk: "Hong Kong", kr: "South Korea",
  in: "India", br: "Brazil", it: "Italy", es: "Spain", se: "Sweden",
  ch: "Switzerland", fi: "Finland", no: "Norway", pl: "Poland", ua: "Ukraine",
  il: "Israel", tw: "Taiwan", ie: "Ireland", at: "Austria", be: "Belgium",
  cz: "Czechia", dk: "Denmark", ro: "Romania", bg: "Bulgaria", hu: "Hungary",
  pt: "Portugal", ae: "UAE", za: "South Africa", mx: "Mexico", ar: "Argentina",
  kz: "Kazakhstan",
};

export function ServerList() {
  const { state, dispatch, toast } = useApp();
  void state.langTick;
  const { search, tab, sortBy, viewMode } = state.serverListUI;
  const setSearch = (v: string) => dispatch({ type: "SET_SERVER_LIST_UI", ui: { search: v } });
  const setTab = (v: string) => dispatch({ type: "SET_SERVER_LIST_UI", ui: { tab: v } });
  const setSortBy = (v: SortKey) => dispatch({ type: "SET_SERVER_LIST_UI", ui: { sortBy: v } });
  const setViewMode = (v: ViewMode) => dispatch({ type: "SET_SERVER_LIST_UI", ui: { viewMode: v } });
  const [pinging, setPinging] = useState(false);
  const [autoSelecting, setAutoSelecting] = useState(false);
  const [showImport, setShowImport] = useState(false);
  const [importText, setImportText] = useState("");
  const [importLoading, setImportLoading] = useState(false);
  const [collapsedCountries, setCollapsedCountries] = useState<Set<string>>(new Set());

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

  const sorted = useMemo(() => {
    const arr = [...filtered];
    arr.sort((a, b) => {
      // Favorites always first
      if (a.favorite && !b.favorite) return -1;
      if (!a.favorite && b.favorite) return 1;

      if (sortBy === "ping") {
        return (a.latency_ms ?? 99999) - (b.latency_ms ?? 99999);
      }
      if (sortBy === "protocol") return a.protocol.localeCompare(b.protocol);
      if (sortBy === "country") {
        const ca = extractCountryCode(a.name) || "zz";
        const cb = extractCountryCode(b.name) || "zz";
        return ca.localeCompare(cb);
      }
      return a.name.localeCompare(b.name);
    });
    return arr;
  }, [filtered, sortBy]);

  // Group by country when sorting by country
  const groupedByCountry = useMemo(() => {
    if (sortBy !== "country") return null;
    const groups: { code: string; name: string; flag: string; servers: ServerInfo[] }[] = [];
    const map = new Map<string, ServerInfo[]>();
    for (const s of sorted) {
      const code = extractCountryCode(s.name) || "other";
      if (!map.has(code)) map.set(code, []);
      map.get(code)!.push(s);
    }
    for (const [code, servers] of map) {
      groups.push({
        code,
        name: COUNTRY_NAMES[code] || code.toUpperCase(),
        flag: COUNTRY_FLAGS[code] || "\u{1F30D}",
        servers,
      });
    }
    return groups;
  }, [sorted, sortBy]);

  const toggleCountry = (code: string) => {
    setCollapsedCountries(prev => {
      const next = new Set(prev);
      if (next.has(code)) next.delete(code);
      else next.add(code);
      return next;
    });
  };

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
    const unlisten = await listen<[string, number | null]>("ping-result", (e) => {
      dispatch({ type: "UPDATE_LATENCIES", results: [e.payload] });
    });
    try {
      const results = await api.pingAllServers();
      const reachable = results.filter(([, ms]) => ms !== null).length;
      toast(`${t("toast.pingDone")}: ${reachable}/${results.length} ${t("toast.reachable")}`, "success");
    } catch (e) {
      toast(`${e}`, "error");
    } finally {
      unlisten();
      setPinging(false);
    }
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
          <div className="view-toggle">
            <button
              className={`view-toggle-btn ${viewMode === "list" ? "active" : ""}`}
              onClick={() => setViewMode("list")}
              title="List"
            >
              <ListIcon size={14} />
            </button>
            <button
              className={`view-toggle-btn ${viewMode === "grid" ? "active" : ""}`}
              onClick={() => setViewMode("grid")}
              title="Grid"
            >
              <GridIcon size={14} />
            </button>
          </div>
          <select
            className="sort-select"
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value as SortKey)}
          >
            <option value="name">{t("servers.sortName")}</option>
            <option value="ping">{t("servers.sortPing")}</option>
            <option value="protocol">{t("servers.sortProto")}</option>
            <option value="country">{t("servers.sortCountry")}</option>
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

      <div className={`server-list-items ${viewMode === "grid" ? "grid-view" : ""}`}>
        {filtered.length === 0 ? (
          <div className="empty-list">
            {state.servers.length === 0 ? t("servers.empty") : t("servers.noMatch")}
          </div>
        ) : groupedByCountry ? (
          groupedByCountry.map((group) => (
            <div key={group.code} className="country-group">
              <div
                className="country-group-header"
                onClick={() => toggleCountry(group.code)}
              >
                <span className="country-flag">{group.flag}</span>
                <span className="country-name">{group.name}</span>
                <span className="country-count">{group.servers.length}</span>
                <span className={`country-chevron ${collapsedCountries.has(group.code) ? "collapsed" : ""}`}>
                  &#9660;
                </span>
              </div>
              {!collapsedCountries.has(group.code) && (
                <div className="country-group-servers">
                  {group.servers.map((s) => (
                    <ServerCard key={s.id} server={s} compact={viewMode === "grid"} />
                  ))}
                </div>
              )}
            </div>
          ))
        ) : (
          sorted.map((s) => <ServerCard key={s.id} server={s} compact={viewMode === "grid"} />)
        )}
      </div>
    </div>
  );
}
