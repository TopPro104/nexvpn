import { useState } from "react";
import { useApp } from "../../context/AppContext";
import { api } from "../../api/tauri";
import { Button } from "../ui/Button";
import { Spinner } from "../ui/Spinner";
import { AddSubModal } from "./AddSubModal";
import { t } from "../../i18n/translations";

function formatDate(ts: number | null): string {
  if (!ts) return t("subs.never");
  return new Date(ts * 1000).toLocaleString();
}

export function SubList() {
  const { state, dispatch, toast } = useApp();
  void state.langTick;
  const [showAdd, setShowAdd] = useState(false);
  const [updatingId, setUpdatingId] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const handleUpdate = async (id: string) => {
    setUpdatingId(id);
    try {
      const newServers = await api.updateSubscription(id);
      const [servers, subs] = await Promise.all([
        api.getServers(),
        api.getSubscriptions(),
      ]);
      dispatch({ type: "SET_SERVERS", servers });
      dispatch({ type: "SET_SUBSCRIPTIONS", subs });
      toast(`${t("toast.subUpdated")}: ${newServers.length} ${t("subs.servers")}`, "success");
    } catch (e) {
      toast(`${e}`, "error");
    }
    setUpdatingId(null);
  };

  const handleDelete = async (id: string) => {
    setDeletingId(id);
    try {
      await api.deleteSubscription(id);
      const [servers, subs] = await Promise.all([
        api.getServers(),
        api.getSubscriptions(),
      ]);
      dispatch({ type: "SET_SERVERS", servers });
      dispatch({ type: "SET_SUBSCRIPTIONS", subs });
      toast(t("toast.subDeleted"), "info");
    } catch (e) {
      toast(`${e}`, "error");
    }
    setDeletingId(null);
  };

  return (
    <div className="sub-page">
      <div className="sub-header">
        <h2>{t("subs.title")}</h2>
        <div style={{ display: "flex", gap: 8 }}>
          <Button
            variant="secondary"
            onClick={() => api.openUrl("https://t.me/vpnhorbot")}
          >
            {t("subs.reliableSource")}
          </Button>
          <Button onClick={() => setShowAdd(true)}>{t("subs.add")}</Button>
        </div>
      </div>

      {state.subscriptions.length === 0 ? (
        <div className="empty-list">{t("subs.empty")}</div>
      ) : (
        <div className="sub-list">
          {state.subscriptions.map((sub) => {
            const isFeatured = sub.url.includes("sub.royaltykey.ru");
            return (
            <div key={sub.id} className={`sub-card${isFeatured ? " sub-featured" : ""}`}>
              <div className="sub-info">
                <div className="sub-name">
                  {isFeatured && <span className="sub-featured-badge">RoyaltyKey</span>}
                  {sub.name}
                </div>
                <div className="sub-url">{sub.url}</div>
                <div className="sub-meta">
                  {sub.server_count} {t("subs.servers")} | {t("subs.updated")}: {formatDate(sub.updated_at)}
                </div>
              </div>
              <div className="sub-actions">
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => handleUpdate(sub.id)}
                  disabled={updatingId === sub.id}
                >
                  {updatingId === sub.id ? <Spinner size={14} /> : t("subs.update")}
                </Button>
                <Button
                  variant="danger"
                  size="sm"
                  onClick={() => handleDelete(sub.id)}
                  disabled={deletingId === sub.id}
                >
                  {deletingId === sub.id ? <Spinner size={14} /> : t("subs.delete")}
                </Button>
              </div>
            </div>
            );
          })}
        </div>
      )}

      <AddSubModal open={showAdd} onClose={() => setShowAdd(false)} />
    </div>
  );
}
