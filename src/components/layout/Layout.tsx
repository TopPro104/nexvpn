import React from "react";
import { Sidebar } from "./Sidebar";
import { useApp, SettingsTab, StatsTab, LogsFilter } from "../../context/AppContext";
import { t } from "../../i18n/translations";

function SubTabs() {
  const { state, dispatch } = useApp();
  void state.langTick;

  switch (state.page) {
    case "home": {
      const tabs: { id: string; label: string }[] = [
        { id: "all", label: `${t("servers.all")} (${state.servers.length})` },
        { id: "manual", label: t("servers.manual") },
        ...state.subscriptions.map((sub) => ({
          id: sub.id,
          label: sub.name,
        })),
      ];
      return (
        <>
          {tabs.map((tt) => (
            <div
              key={tt.id}
              className={`tab ${state.serverListUI.tab === tt.id ? "active" : ""}`}
              onClick={() => dispatch({ type: "SET_SERVER_LIST_UI", ui: { tab: tt.id } })}
            >
              {tt.label}
            </div>
          ))}
        </>
      );
    }

    case "settings": {
      const isRu = !t("servers.manual").includes("Manual");
      const tabs: { id: SettingsTab; label: string }[] = [
        { id: "style", label: isRu ? "Стиль" : "Style" },
        { id: "vpn", label: "VPN" },
        { id: "other", label: isRu ? "Прочее" : "Other" },
      ];
      return (
        <>
          {tabs.map((tt) => (
            <div
              key={tt.id}
              className={`tab ${state.settingsTab === tt.id ? "active" : ""}`}
              onClick={() => dispatch({ type: "SET_SETTINGS_TAB", tab: tt.id })}
            >
              {tt.label}
            </div>
          ))}
        </>
      );
    }

    case "stats": {
      const tabs: { id: StatsTab; label: string }[] = [
        { id: "overview", label: "Overview" },
        { id: "traffic", label: "Traffic" },
        { id: "history", label: "History" },
      ];
      return (
        <>
          {tabs.map((tt) => (
            <div
              key={tt.id}
              className={`tab ${state.statsTab === tt.id ? "active" : ""}`}
              onClick={() => dispatch({ type: "SET_STATS_TAB", tab: tt.id })}
            >
              {tt.label}
            </div>
          ))}
        </>
      );
    }

    case "logs": {
      const tabs: { id: LogsFilter; label: string }[] = [
        { id: "all", label: "All" },
        { id: "errors", label: "Errors" },
        { id: "warnings", label: "Warnings" },
      ];
      return (
        <>
          {tabs.map((tt) => (
            <div
              key={tt.id}
              className={`tab ${state.logsFilter === tt.id ? "active" : ""}`}
              onClick={() => dispatch({ type: "SET_LOGS_FILTER", filter: tt.id })}
            >
              {tt.label}
            </div>
          ))}
        </>
      );
    }

    default:
      return null;
  }
}

export function Layout({ children }: { children: React.ReactNode }) {
  const { state } = useApp();

  return (
    <div className="app-layout">
      <div className="top-bar">
        <div className="brand-box">
          <div className="logo">N</div>
          <div className="brand-text">
            <h1>NexVPN</h1>
            <p>Secure tunneling</p>
          </div>
        </div>
        <div className="tabs-wrapper">
          <SubTabs />
        </div>
        <div className={`conn-badge ${state.connected ? "on" : "off"}`}>
          <span className="conn-badge-dot" />
          <span className="conn-badge-text">
            {state.connected
              ? (state.serverName || "Connected")
              : "Disconnected"}
          </span>
        </div>
      </div>
      <div className="workspace">
        <Sidebar />
        <main className="main-content">{children}</main>
      </div>
    </div>
  );
}
