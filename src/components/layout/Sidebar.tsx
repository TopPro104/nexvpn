import { useApp, Page } from "../../context/AppContext";
import { t } from "../../i18n/translations";
import {
  ZapIcon,
  FolderIcon,
  BarChartIcon,
  FileTextIcon,
  SettingsIcon,
  RouteIcon,
} from "../ui/Icons";
import type { ReactNode } from "react";

const navIcons: Record<Page, ReactNode> = {
  home: <ZapIcon size={18} />,
  subscriptions: <FolderIcon size={18} />,
  routing: <RouteIcon size={18} />,
  stats: <BarChartIcon size={18} />,
  logs: <FileTextIcon size={18} />,
  settings: <SettingsIcon size={18} />,
};

export function Sidebar() {
  const { state, dispatch } = useApp();
  // langTick dependency ensures re-render on language change
  void state.langTick;

  const navItems: { id: Page; label: string }[] = [
    { id: "home", label: t("nav.home") },
    { id: "subscriptions", label: t("nav.subscriptions") },
    { id: "routing", label: t("nav.routing") },
    { id: "stats", label: t("nav.stats") },
    { id: "logs", label: t("nav.logs") },
    { id: "settings", label: t("nav.settings") },
  ];

  return (
    <aside className="sidebar">
      <div className="sidebar-brand">
        <span className="brand-icon">N</span>
        <span className="brand-text">NexVPN</span>
      </div>
      <nav className="sidebar-nav">
        {navItems.map((item) => (
          <button
            key={item.id}
            className={`nav-item ${state.page === item.id ? "active" : ""}`}
            onClick={() => dispatch({ type: "SET_PAGE", page: item.id })}
          >
            <span className="nav-icon">
              {navIcons[item.id]}
            </span>
            <span className="nav-label">{item.label}</span>
          </button>
        ))}
      </nav>
      <div className="sidebar-footer">
        <div className={`status-indicator ${state.connected ? "on" : "off"}`} />
        <span className="sidebar-status">
          {state.connected ? t("status.connected") : t("status.disconnected")}
        </span>
      </div>
    </aside>
  );
}
