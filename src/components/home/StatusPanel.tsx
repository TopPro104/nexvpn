import { useState, useEffect } from "react";
import { useApp } from "../../context/AppContext";
import { api } from "../../api/tauri";
import { Spinner } from "../ui/Spinner";
import { t } from "../../i18n/translations";
import { PowerIcon } from "../ui/Icons";

export function StatusPanel() {
  const { state, dispatch, toast } = useApp();
  void state.langTick;
  const [loading, setLoading] = useState(false);
  const [elapsed, setElapsed] = useState(0);

  // Calculate elapsed from global connectedAt timestamp — survives page switches
  useEffect(() => {
    if (!state.connectedAt) {
      setElapsed(0);
      return;
    }

    const tick = () => {
      setElapsed(Math.floor((Date.now() - state.connectedAt!) / 1000));
    };
    tick(); // immediate update
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [state.connectedAt]);

  const formatTime = (seconds: number) => {
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = seconds % 60;
    return `${h.toString().padStart(2, "0")}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  };

  const handleToggle = async () => {
    if (loading) return;
    setLoading(true);
    try {
      if (state.connected) {
        const status = await api.disconnect();
        dispatch({ type: "SET_STATUS", status });
        toast(t("toast.disconnected"), "info");
      } else {
        if (!state.selectedServerId) {
          toast(t("servers.selectFirst"), "error");
          setLoading(false);
          return;
        }
        const status = await api.connect(state.selectedServerId);
        dispatch({ type: "SET_STATUS", status });
        toast(`${t("toast.connectedTo")} ${status.server_name}`, "success");
      }
    } catch (e) {
      toast(`${e}`, "error");
    }
    setLoading(false);
  };

  const selectedServer = state.servers.find(
    (s) => s.id === state.selectedServerId
  );

  return (
    <div className="status-panel">
      <button
        className={`connect-btn ${state.connected ? "connected" : ""} ${loading ? "loading" : ""}`}
        onClick={handleToggle}
        disabled={loading}
      >
        <div className="connect-btn-inner">
          {loading ? (
            <Spinner size={32} />
          ) : (
            <span className="connect-btn-icon">
              <PowerIcon size={40} />
            </span>
          )}
        </div>
      </button>
      <div className="status-info">
        <div className="status-label">
          {state.connected ? t("status.connected") : t("status.disconnected")}
        </div>
        {state.connected && state.serverName && (
          <div className="status-server">{state.serverName}</div>
        )}
        {!state.connected && selectedServer && (
          <div className="status-server selected">
            {selectedServer.name} ({selectedServer.protocol})
          </div>
        )}
        {state.connected && (
          <div className="status-time">{formatTime(elapsed)}</div>
        )}
        <div className="status-core">
          {state.coreType} | SOCKS:{state.socksPort} HTTP:{state.httpPort}
          <span className={`vpn-mode-badge ${state.settings.vpn_mode}`}>
            {state.settings.vpn_mode === "tun" ? "TUN" : "Proxy"}
          </span>
        </div>
      </div>
    </div>
  );
}
