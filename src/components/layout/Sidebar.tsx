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
  home: <ZapIcon size={20} />,
  subscriptions: <FolderIcon size={20} />,
  routing: <RouteIcon size={20} />,
  stats: <BarChartIcon size={20} />,
  logs: <FileTextIcon size={20} />,
  settings: <SettingsIcon size={20} />,
};

export function Sidebar() {
  const { state, dispatch } = useApp();
  void state.langTick;

  const group1: { id: Page; label: string }[] = [
    { id: "home",          label: t("nav.home") },
    { id: "subscriptions", label: t("nav.subscriptions") },
    { id: "routing",       label: t("nav.routing") },
  ];

  const group2: { id: Page; label: string }[] = [
    { id: "stats", label: t("nav.stats") },
    { id: "logs",  label: t("nav.logs") },
  ];

  const group3: { id: Page; label: string }[] = [
    { id: "settings", label: t("nav.settings") },
  ];

  const renderGroup = (items: typeof group1) =>
    items.map((item) => (
      <button
        key={item.id}
        className={`nav-item ${state.page === item.id ? "active" : ""}`}
        onClick={() => dispatch({ type: "SET_PAGE", page: item.id })}
        title={item.label}
      >
        {navIcons[item.id]}
      </button>
    ));

  return (
    <aside className="sidebar">
      {renderGroup(group1)}
      <div className="nav-separator" />
      {renderGroup(group2)}
      <div className="nav-separator" />
      {renderGroup(group3)}
    </aside>
  );
}
