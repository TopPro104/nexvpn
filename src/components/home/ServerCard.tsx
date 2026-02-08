import { useApp } from "../../context/AppContext";
import { ServerInfo, api } from "../../api/tauri";
import { Flag } from "../ui/Flag";
import { extractCountryCode } from "../../utils/countryUtils";

function pingClass(ms: number | null): string {
  if (ms == null) return "";
  if (ms < 150) return "good";
  if (ms < 300) return "medium";
  return "bad";
}

export function ServerCard({ server }: { server: ServerInfo }) {
  const { state, dispatch, toast } = useApp();
  const isSelected = state.selectedServerId === server.id;
  const country = extractCountryCode(server.name);

  const handleSelect = () => {
    dispatch({ type: "SELECT_SERVER", id: server.id });
  };

  const handleDelete = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await api.removeServer(server.id);
      dispatch({ type: "REMOVE_SERVER", id: server.id });
    } catch (err) {
      toast(`${err}`, "error");
    }
  };

  return (
    <div
      className={`server-card ${isSelected ? "active" : ""}`}
      onClick={handleSelect}
    >
      <Flag code={country} size={22} />
      <span className="server-proto">{server.protocol}</span>
      <span className="server-name">{server.name}</span>
      <span className={`server-ping ${pingClass(server.latency_ms)}`}>
        {server.latency_ms != null ? `${server.latency_ms}ms` : "\u2014"}
      </span>
      <span className="server-delete" onClick={handleDelete} title="Remove">
        &times;
      </span>
    </div>
  );
}
