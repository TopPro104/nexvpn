import { useState } from "react";
import { useApp } from "../../context/AppContext";
import { ServerInfo, api } from "../../api/tauri";
import { Flag } from "../ui/Flag";
import { extractCountryCode } from "../../utils/countryUtils";
import { StarIcon, StarFilledIcon } from "../ui/Icons";
import { t } from "../../i18n/translations";

const PROTO_COLORS: Record<string, string> = {
  vless: "#5b9aff",
  vmess: "#a47cff",
  trojan: "#ff6b6b",
  shadowsocks: "#44cc88",
  ss: "#44cc88",
  hysteria2: "#ff9f43",
  hysteria: "#ff9f43",
  tuic: "#00d2ff",
  naive: "#e84393",
  wireguard: "#88c057",
};

function getProtoColor(proto: string): string {
  return PROTO_COLORS[proto.toLowerCase()] || "var(--text-muted)";
}

function pingPercent(ms: number | null): number {
  if (ms == null) return 0;
  if (ms < 50) return 100;
  if (ms > 500) return 5;
  return Math.round(100 - ((ms - 50) / 450) * 95);
}

function pingClass(ms: number | null): string {
  if (ms == null) return "";
  if (ms < 150) return "good";
  if (ms < 300) return "medium";
  return "bad";
}

export function ServerCard({ server, compact }: { server: ServerInfo; compact?: boolean }) {
  const { state, dispatch, toast } = useApp();
  const isSelected = state.selectedServerId === server.id;
  const country = extractCountryCode(server.name);
  const [favLoading, setFavLoading] = useState(false);
  const protoColor = getProtoColor(server.protocol);
  const pClass = pingClass(server.latency_ms);
  const pPct = pingPercent(server.latency_ms);

  const handleSelect = () => {
    dispatch({ type: "SELECT_SERVER", id: server.id });
  };

  const handleDoubleClick = async () => {
    dispatch({ type: "SELECT_SERVER", id: server.id });
    if (!state.connected || state.selectedServerId !== server.id) {
      try {
        const status = await api.connect(server.id);
        dispatch({ type: "SET_STATUS", status });
        toast(`${t("toast.connectedTo")} ${server.name}`, "success");
      } catch (err) {
        toast(err instanceof Error ? err.message : String(err), "error");
      }
    }
  };

  const handleDelete = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!window.confirm(`${t("servers.confirmDelete")} "${server.name}"?`)) return;
    try {
      await api.removeServer(server.id);
      dispatch({ type: "REMOVE_SERVER", id: server.id });
    } catch (err) {
      toast(err instanceof Error ? err.message : String(err), "error");
    }
  };

  const handleFavorite = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (favLoading) return;
    setFavLoading(true);
    try {
      await api.toggleFavorite(server.id);
      const servers = await api.getServers();
      dispatch({ type: "SET_SERVERS", servers });
    } catch (err) {
      toast(`${err}`, "error");
    }
    setFavLoading(false);
  };

  if (compact) {
    return (
      <div
        className={`server-card grid-card ${isSelected ? "active" : ""}`}
        onClick={handleSelect}
        onDoubleClick={handleDoubleClick}
      >
        <div className="grid-card-top">
          <Flag code={country} size={20} />
          <span className="server-fav" onClick={handleFavorite}>
            {server.favorite ? <StarFilledIcon size={14} color="var(--warning)" /> : <StarIcon size={14} />}
          </span>
        </div>
        <div className="grid-card-name">{server.name}</div>
        <div className="grid-card-meta">
          <span className="proto-badge" style={{ borderColor: protoColor, color: protoColor }}>
            {server.protocol}
          </span>
          <span className={`server-ping ${pClass}`}>
            {server.latency_ms != null ? `${server.latency_ms}ms` : "\u2014"}
          </span>
        </div>
        {server.latency_ms != null && (
          <div className="ping-bar-container">
            <div className={`ping-bar ${pClass}`} style={{ width: `${pPct}%` }} />
          </div>
        )}
      </div>
    );
  }

  return (
    <div
      className={`server-card ${isSelected ? "active" : ""}`}
      onClick={handleSelect}
      onDoubleClick={handleDoubleClick}
    >
      <span className="server-fav" onClick={handleFavorite}>
        {server.favorite ? <StarFilledIcon size={14} color="var(--warning)" /> : <StarIcon size={14} />}
      </span>
      <Flag code={country} size={22} />
      <span className="proto-badge" style={{ borderColor: protoColor, color: protoColor }}>
        {server.protocol}
      </span>
      <span className="server-name">{server.name}</span>
      <div className="ping-section">
        {server.latency_ms != null && (
          <div className="ping-bar-container">
            <div className={`ping-bar ${pClass}`} style={{ width: `${pPct}%` }} />
          </div>
        )}
        <span className={`server-ping ${pClass}`}>
          {server.latency_ms != null ? `${server.latency_ms}ms` : "\u2014"}
        </span>
      </div>
      <span className="server-delete" onClick={handleDelete} title="Remove">
        &times;
      </span>
    </div>
  );
}
