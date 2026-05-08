import { useState } from "react";
import { useApp } from "../../context/AppContext";
import { api, SubscriptionInfo } from "../../api/tauri";
import { Button } from "../ui/Button";
import { Spinner } from "../ui/Spinner";
import { AddSubModal } from "./AddSubModal";
import { t } from "../../i18n/translations";
import { showConfirm } from "../../utils/confirm";
import {
  BarChartIcon,
  CalendarIcon,
  RefreshCwIcon,
  LinkIcon,
  MegaphoneIcon,
  ChevronDownIcon,
  AlertTriangleIcon,
  XCircleIcon,
} from "../ui/Icons";

function formatDate(ts: number | null): string {
  if (!ts) return t("subs.never");
  return new Date(ts * 1000).toLocaleString();
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return (bytes / Math.pow(1024, i)).toFixed(i > 1 ? 2 : 0) + " " + units[i];
}

function SubExpanded({ sub }: { sub: SubscriptionInfo }) {
  const now = Date.now() / 1000;
  const hasTraffic = sub.download != null || sub.upload != null;
  const used = (sub.download ?? 0) + (sub.upload ?? 0);
  const total = sub.total ?? 0;
  const isUnlimited = total === 0;
  const percent = isUnlimited ? 0 : Math.min((used / total) * 100, 100);
  const hasExpiry = sub.expire != null && sub.expire > 0;
  const isExpired = hasExpiry && sub.expire! < now;
  const daysLeft = hasExpiry ? Math.ceil((sub.expire! - now) / 86400) : 0;

  const hasAnything = hasTraffic || hasExpiry || sub.refill_date || sub.support_url || sub.web_page_url || sub.announce;
  if (!hasAnything) return null;

  return (
    <div className="sub-expanded">
      {hasTraffic && (
        <div className="sub-row">
          <BarChartIcon size={14} className="sub-row-icon" />
          <span className="sub-row-label">{t("subs.traffic")}</span>
          <span className="sub-row-value">
            {formatBytes(used)}
            {!isUnlimited && <span className="sub-row-dim"> / {formatBytes(total)}</span>}
            {isUnlimited && <span className="sub-badge accent">{t("subs.unlimited")}</span>}
          </span>
        </div>
      )}
      {hasTraffic && !isUnlimited && (
        <div className="sub-progress" role="progressbar" aria-valuenow={Math.round(percent)} aria-valuemin={0} aria-valuemax={100}>
          <div
            className={`sub-progress-fill${percent > 90 ? " danger" : percent > 70 ? " warning" : ""}`}
            style={{ width: `${percent}%` }}
          />
        </div>
      )}

      {hasExpiry && (
        <div className="sub-row">
          {isExpired
            ? <XCircleIcon size={14} className="sub-row-icon text-danger" />
            : daysLeft <= 3
              ? <AlertTriangleIcon size={14} className="sub-row-icon text-warning" />
              : <CalendarIcon size={14} className="sub-row-icon" />
          }
          <span className="sub-row-label">{isExpired ? t("subs.expired") : t("subs.expires")}</span>
          <span className={`sub-row-value${isExpired ? " text-danger" : daysLeft <= 3 ? " text-warning" : ""}`}>
            {formatDate(sub.expire)}
            {!isExpired && <span className={`sub-badge${daysLeft <= 3 ? " warning" : ""}`}>{daysLeft}d</span>}
          </span>
        </div>
      )}

      {sub.refill_date != null && (
        <div className="sub-row">
          <RefreshCwIcon size={14} className="sub-row-icon" />
          <span className="sub-row-label">{t("subs.refill")}</span>
          <span className="sub-row-value">{formatDate(sub.refill_date)}</span>
        </div>
      )}

      {(sub.support_url || sub.web_page_url) && (
        <div className="sub-row">
          <LinkIcon size={14} className="sub-row-icon" />
          <div className="sub-row-links">
            {sub.web_page_url && (
              <button className="sub-row-link" onClick={() => api.openUrl(sub.web_page_url!)}>{t("subs.manage")}</button>
            )}
            {sub.support_url && (
              <button className="sub-row-link" onClick={() => api.openUrl(sub.support_url!)}>{t("subs.support")}</button>
            )}
          </div>
        </div>
      )}

      {sub.announce && (
        <div className="sub-announce">
          <MegaphoneIcon size={14} className="sub-row-icon" />
          <span>{sub.announce}</span>
        </div>
      )}
    </div>
  );
}

export function SubList() {
  const { state, dispatch, toast } = useApp();
  void state.langTick;
  const [showAdd, setShowAdd] = useState(false);
  const [updatingId, setUpdatingId] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [updatingAll, setUpdatingAll] = useState(false);

  const handleUpdateAll = async () => {
    if (updatingAll || state.subscriptions.length === 0) return;
    setUpdatingAll(true);
    try {
      const results = await Promise.allSettled(
        state.subscriptions.map((s) => api.updateSubscription(s.id))
      );
      const [servers, subs] = await Promise.all([
        api.getServers(),
        api.getSubscriptions(),
      ]);
      dispatch({ type: "SET_SERVERS", servers });
      dispatch({ type: "SET_SUBSCRIPTIONS", subs });
      const ok = results.filter((r) => r.status === "fulfilled").length;
      const total = results.length;
      const firstErr = results.find((r) => r.status === "rejected") as PromiseRejectedResult | undefined;
      if (ok === total) {
        toast(`${t("toast.subsUpdatedAll")}: ${ok}/${total}`, "success");
      } else if (ok === 0) {
        toast(firstErr ? String(firstErr.reason) : "Failed", "error");
      } else {
        toast(`${t("toast.subsUpdatedAll")}: ${ok}/${total}`, "info");
      }
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), "error");
    }
    setUpdatingAll(false);
  };

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
      toast(e instanceof Error ? e.message : String(e), "error");
    }
    setUpdatingId(null);
  };

  const handleDelete = async (id: string) => {
    const sub = state.subscriptions.find((s) => s.id === id);
    if (!sub) return;
    if (!await showConfirm(`${t("subs.confirmDelete")} "${sub.name}"?\n${sub.server_count} ${t("subs.servers")} ${t("subs.willBeRemoved")}`)) return;

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
      toast(e instanceof Error ? e.message : String(e), "error");
    }
    setDeletingId(null);
  };

  const hasDetails = (sub: SubscriptionInfo) =>
    sub.download != null || sub.upload != null || (sub.expire != null && sub.expire > 0) ||
    sub.refill_date != null || sub.support_url || sub.web_page_url || sub.announce;

  return (
    <div className="sub-page">
      <div className="sub-header">
        <h2>{t("subs.title")}</h2>
        <div className="sub-header-actions">
          <Button
            variant="secondary"
            onClick={() => api.openUrl("https://t.me/vpnhorbot")}
          >
            {t("subs.reliableSource")}
          </Button>
          {state.subscriptions.length > 1 && (
            <Button
              variant="secondary"
              onClick={handleUpdateAll}
              disabled={updatingAll || updatingId !== null}
            >
              {updatingAll ? <Spinner size={14} /> : t("subs.updateAll")}
            </Button>
          )}
          <Button onClick={() => setShowAdd(true)}>{t("subs.add")}</Button>
        </div>
      </div>

      {state.subscriptions.length === 0 ? (
        <div className="empty-list">{t("subs.empty")}</div>
      ) : (
        <div className="sub-list">
          {state.subscriptions.map((sub) => {
            const isFeatured = sub.url.includes("sub.royaltykey.ru");
            const isExpanded = expandedId === sub.id;
            const expandable = hasDetails(sub);
            return (
            <div key={sub.id} className={`sub-card${isFeatured ? " sub-featured" : ""}${isExpanded ? " expanded" : ""}`}>
              <div className="sub-top">
                <div
                  className={`sub-info${expandable ? " clickable" : ""}`}
                  onClick={() => expandable && setExpandedId(isExpanded ? null : sub.id)}
                  onKeyDown={(e) => expandable && (e.key === "Enter" || e.key === " ") && (e.preventDefault(), setExpandedId(isExpanded ? null : sub.id))}
                  role={expandable ? "button" : undefined}
                  tabIndex={expandable ? 0 : undefined}
                  aria-expanded={expandable ? isExpanded : undefined}
                >
                  <div className="sub-name-row">
                    <div className="sub-name">
                      {isFeatured && <span className="sub-featured-badge">RoyaltyKey</span>}
                      {sub.name}
                    </div>
                    {expandable && (
                      <ChevronDownIcon size={14} className={`sub-chevron${isExpanded ? " open" : ""}`} />
                    )}
                  </div>
                  <div className="sub-compact-meta">
                    <span>{sub.server_count} {t("subs.servers")}</span>
                    <span className="sub-sep">·</span>
                    <span>{t("subs.updated")}: {formatDate(sub.updated_at)}</span>
                    {sub.update_interval != null && (
                      <>
                        <span className="sub-sep">·</span>
                        <span className="sub-auto-tag">
                          <RefreshCwIcon size={10} />
                          {sub.update_interval}{t("subs.hours")}
                        </span>
                      </>
                    )}
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
              {isExpanded && <SubExpanded sub={sub} />}
            </div>
            );
          })}
        </div>
      )}

      <AddSubModal open={showAdd} onClose={() => setShowAdd(false)} />
    </div>
  );
}
