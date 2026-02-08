import { useApp, Page } from "../../context/AppContext";
import { t } from "../../i18n/translations";

export function Sidebar() {
  const { state, dispatch } = useApp();
  // langTick dependency ensures re-render on language change
  void state.langTick;

  const navItems: { id: Page; label: string; icon: string }[] = [
    { id: "home", label: t("nav.home"), icon: "\u26A1" },
    { id: "subscriptions", label: t("nav.subscriptions"), icon: "\uD83D\uDCC1" },
    { id: "stats", label: t("nav.stats"), icon: "\uD83D\uDCCA" },
    { id: "logs", label: t("nav.logs"), icon: "\uD83D\uDCDD" },
    { id: "settings", label: t("nav.settings"), icon: "\u2699\uFE0F" },
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
            <span className="nav-icon">{item.icon}</span>
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
