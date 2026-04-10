import { useState, useEffect, useMemo } from "react";
import { useApp } from "../../context/AppContext";
import { api, InstalledApp } from "../../api/tauri";
import { t } from "../../i18n/translations";

export function PerAppVpn() {
  const { state, dispatch, toast } = useApp();
  const [apps, setApps] = useState<InstalledApp[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.getInstalledApps()
      .then(setApps)
      .catch(() => toast("Failed to load apps", "error"))
      .finally(() => setLoading(false));
  }, [toast]);

  const mode = state.settings.per_app_mode;
  const selected = new Set(state.settings.per_app_list);

  const filtered = useMemo(() => {
    if (!search) return apps;
    const q = search.toLowerCase();
    return apps.filter(
      (a) =>
        a.label.toLowerCase().includes(q) ||
        a.package_name.toLowerCase().includes(q)
    );
  }, [apps, search]);

  const toggleApp = (pkg: string) => {
    const newList = selected.has(pkg)
      ? state.settings.per_app_list.filter((p) => p !== pkg)
      : [...state.settings.per_app_list, pkg];

    const newSettings = { ...state.settings, per_app_list: newList };
    dispatch({ type: "SET_SETTINGS", settings: newSettings });
    api.saveSettings(newSettings).catch((e) => toast(`${e}`, "error"));
  };

  const setMode = (newMode: string) => {
    const newSettings = { ...state.settings, per_app_mode: newMode };
    dispatch({ type: "SET_SETTINGS", settings: newSettings });
    api.saveSettings(newSettings).catch((e) => toast(`${e}`, "error"));
  };

  if (loading) {
    return <div className="per-app-loading">{t("perApp.loading")}</div>;
  }

  return (
    <div className="per-app-vpn">
      <div className="settings-section">
        <div className="settings-label">{t("perApp.mode")}</div>
        <div className="core-radio-group">
          <label className={`core-radio ${mode === "all" ? "active" : ""}`}>
            <input type="radio" name="perAppMode" checked={mode === "all"} onChange={() => setMode("all")} />
            <span>{t("perApp.all")}</span>
          </label>
          <label className={`core-radio ${mode === "include" ? "active" : ""}`}>
            <input type="radio" name="perAppMode" checked={mode === "include"} onChange={() => setMode("include")} />
            <span>{t("perApp.include")}</span>
          </label>
          <label className={`core-radio ${mode === "exclude" ? "active" : ""}`}>
            <input type="radio" name="perAppMode" checked={mode === "exclude"} onChange={() => setMode("exclude")} />
            <span>{t("perApp.exclude")}</span>
          </label>
        </div>
        {mode !== "all" && (
          <div className="hwid-desc">
            {mode === "include" ? t("perApp.includeDesc") : t("perApp.excludeDesc")}
          </div>
        )}
      </div>

      {mode !== "all" && (
        <>
          <div className="per-app-search">
            <input
              className="form-input"
              type="text"
              placeholder={t("perApp.search")}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
            {selected.size > 0 && (
              <span className="per-app-count">{selected.size} {t("perApp.selected")}</span>
            )}
          </div>

          <div className="per-app-list">
            {filtered.map((app) => (
              <div
                key={app.package_name}
                className={`per-app-item ${selected.has(app.package_name) ? "selected" : ""}`}
                onClick={() => toggleApp(app.package_name)}
              >
                {app.icon ? (
                  <img
                    className="per-app-icon"
                    src={`data:image/png;base64,${app.icon}`}
                    alt=""
                  />
                ) : (
                  <div className="per-app-icon per-app-icon-placeholder" />
                )}
                <div className="per-app-info">
                  <div className="per-app-label">{app.label}</div>
                  <div className="per-app-package">{app.package_name}</div>
                </div>
                <div className={`per-app-check ${selected.has(app.package_name) ? "checked" : ""}`}>
                  {selected.has(app.package_name) ? "\u2713" : ""}
                </div>
              </div>
            ))}
            {filtered.length === 0 && (
              <div className="per-app-empty">{t("perApp.noApps")}</div>
            )}
          </div>
        </>
      )}
    </div>
  );
}
