import { useApp } from "../../context/AppContext";
import { api } from "../../api/tauri";
import { Flag } from "../ui/Flag";
import { extractCountryCode } from "../../utils/countryUtils";
import { ZapIcon } from "../ui/Icons";
import { t } from "../../i18n/translations";

export function QuickConnect() {
  const { state, dispatch, toast } = useApp();
  void state.langTick;

  // Get recent servers (that still exist)
  const recentServers = state.recentServerIds
    .map(id => state.servers.find(s => s.id === id))
    .filter(Boolean)
    .slice(0, 2);

  // Find recommended server: lowest ping that's not currently selected
  const recommended = [...state.servers]
    .filter(s => s.latency_ms != null && s.id !== state.selectedServerId)
    .sort((a, b) => (a.latency_ms ?? 99999) - (b.latency_ms ?? 99999))[0];

  // Don't show if no servers at all
  if (state.servers.length === 0) return null;
  // Don't show if no recent servers and no recommended
  if (recentServers.length === 0 && !recommended) return null;

  const handleQuickSelect = async (serverId: string) => {
    dispatch({ type: "SELECT_SERVER", id: serverId });
    if (!state.connected) {
      try {
        const status = await api.connect(serverId);
        dispatch({ type: "SET_STATUS", status });
        toast(`${t("toast.connectedTo")} ${status.server_name}`, "success");
      } catch (e) {
        toast(`${e}`, "error");
      }
    }
  };

  return (
    <div className="quick-connect">
      {recentServers.length > 0 && (
        <div className="quick-section">
          <span className="quick-label">{t("quick.recent")}</span>
          <div className="quick-items">
            {recentServers.map(server => (
              <button
                key={server!.id}
                className={`quick-item ${state.selectedServerId === server!.id ? "active" : ""}`}
                onClick={() => handleQuickSelect(server!.id)}
              >
                <Flag code={extractCountryCode(server!.name)} size={16} />
                <span className="quick-item-name">{server!.name}</span>
                {server!.latency_ms != null && (
                  <span className="quick-item-ping">{server!.latency_ms}ms</span>
                )}
              </button>
            ))}
          </div>
        </div>
      )}
      {recommended && (
        <button
          className="quick-recommended"
          onClick={() => handleQuickSelect(recommended.id)}
        >
          <ZapIcon size={14} />
          <span>{t("quick.fastest")}: {recommended.name} ({recommended.latency_ms}ms)</span>
        </button>
      )}
    </div>
  );
}
