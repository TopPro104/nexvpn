import { useState, useEffect, useCallback, useRef, KeyboardEvent } from "react";
import { useApp } from "../../context/AppContext";
import { api, RoutingProfile } from "../../api/tauri";
import { t, TranslationKey } from "../../i18n/translations";
import { showConfirm } from "../../utils/confirm";
import { Button } from "../ui/Button";
import { Spinner } from "../ui/Spinner";
import { Modal } from "../ui/Modal";
import {
  AlertTriangleIcon,
  CheckCircleIcon,
  ChevronDownIcon,
  CopyIcon,
  DownloadCloudIcon,
  EditIcon,
  FolderIcon,
  PlusIcon,
  RefreshCwIcon,
  TrashIcon,
} from "../ui/Icons";
import { GeoPreviewModal, GeoPreviewTarget, RoutingProfileEditor, isGeoEntry } from "./RoutingProfileEditor";

type AppApi = ReturnType<typeof useApp>;
type EntryKind = "direct" | "proxy" | "block";

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** Built-in Happ profile: Russian sites and IPs direct, everything else via proxy. */
export const BUILTIN_RU_PROFILE = {
  Name: "NexVPN RU",
  GlobalProxy: "true",
  RouteOrder: "block-proxy-direct",
  RemoteDNSType: "DoH",
  RemoteDNSDomain: "https://1.1.1.1/dns-query",
  RemoteDNSIP: "1.1.1.1",
  DomesticDNSType: "DoU",
  DomesticDNSDomain: "",
  DomesticDNSIP: "77.88.8.8",
  Geoipurl: "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geoip.dat",
  Geositeurl: "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geosite.dat",
  DnsHosts: {},
  DirectSites: ["geosite:private", "geosite:category-ru"],
  DirectIp: ["geoip:private", "geoip:ru"],
  ProxySites: [],
  ProxyIp: [],
  BlockSites: [],
  BlockIp: [],
  DomainStrategy: "IPIfNonMatch",
  FakeDNS: "false",
};

/**
 * Imports a routing profile (happ:// / nexvpn:// link or raw JSON) and toasts the result.
 * Tracked in global state so the "downloading geo files" hint survives page switches,
 * and the Routing page reloads its list when it finishes. Returns true on success.
 */
export async function runRoutingImport(
  input: string,
  dispatch: AppApi["dispatch"],
  toast: AppApi["toast"]
): Promise<boolean> {
  dispatch({ type: "ROUTING_IMPORT_PENDING", delta: 1 });
  try {
    const profile = await api.importRoutingProfile(input);
    if (profile === null) {
      toast(t("routing.profileTurnedOff"), "info");
    } else {
      let active = false;
      try {
        active = (await api.getRoutingProfiles()).active_id === profile.id;
      } catch {
        // Only affects the toast wording
      }
      const key = active ? "routing.profileImportedActive" : "routing.profileImported";
      toast(`${t(key)}: ${profile.name}`, "success");
    }
    return true;
  } catch (e) {
    toast(errMsg(e), "error");
    return false;
  } finally {
    dispatch({ type: "ROUTING_IMPORT_PENDING", delta: -1 });
    dispatch({ type: "BUMP_ROUTING_PROFILES" });
  }
}

/** Loads routing profiles and handles (optimistic) activation. */
export function useRoutingProfiles() {
  const { state, toast } = useApp();
  const [profiles, setProfiles] = useState<RoutingProfile[]>([]);
  const [serverActiveId, setServerActiveId] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  // Target of an in-flight activation (id or null = off); undefined when idle
  const [pendingId, setPendingId] = useState<string | null | undefined>(undefined);
  const pendingRef = useRef(false);
  const loadSeq = useRef(0);

  const reload = useCallback(async () => {
    const seq = ++loadSeq.current;
    try {
      const res = await api.getRoutingProfiles();
      if (seq !== loadSeq.current) return; // superseded by a newer load
      setProfiles(res.profiles);
      setServerActiveId(res.active_id);
      setLoadError(null);
    } catch (e) {
      if (seq !== loadSeq.current) return;
      setLoadError(errMsg(e));
    } finally {
      if (seq === loadSeq.current) setLoaded(true);
    }
  }, []);

  useEffect(() => {
    reload();
  }, [reload, state.routingProfilesTick]);

  const shownId = pendingId !== undefined ? pendingId : serverActiveId;
  const activeProfile = (shownId && profiles.find((p) => p.id === shownId)) || null;

  const activate = async (id: string | null) => {
    if (pendingRef.current || id === (activeProfile?.id ?? null)) return;
    const name = profiles.find((p) => p.id === id)?.name ?? "";
    pendingRef.current = true;
    setPendingId(id);
    try {
      await api.setActiveRoutingProfile(id);
      setServerActiveId(id);
      toast(id ? `${t("routing.profileActivated")}: ${name}` : t("routing.profileTurnedOff"), "success");
    } catch (e) {
      toast(errMsg(e), "error");
    } finally {
      pendingRef.current = false;
      setPendingId(undefined);
      reload();
    }
  };

  const replaceProfile = (updated: RoutingProfile) =>
    setProfiles((list) => list.map((p) => (p.id === updated.id ? updated : p)));

  return {
    profiles,
    loaded,
    loadError,
    activeProfile,
    pendingId,
    activate,
    reload,
    replaceProfile,
  };
}

export type RoutingProfilesApi = ReturnType<typeof useRoutingProfiles>;

// ── Helpers ─────────────────────────────────────

const ORDER_LABELS: Record<string, TranslationKey> = {
  block: "routing.catBlock",
  proxy: "routing.catProxy",
  direct: "routing.catDirect",
};

function formatRouteOrder(order: string): string {
  if (!order) return "—";
  return order
    .split("-")
    .map((part) => {
      const key = ORDER_LABELS[part.trim().toLowerCase()];
      return key ? t(key) : part;
    })
    .join(" → ");
}

function formatDns(type: string, domain: string, ip: string): string {
  return [type, domain, ip].filter((x) => x && x.trim()).join(" · ") || "—";
}

function formatTime(ts: number): string {
  return new Date(ts * 1000).toLocaleString();
}

const count = (...lists: (string[] | undefined)[]) =>
  lists.reduce((n, l) => n + (l?.length ?? 0), 0);

type ListKey = "direct_sites" | "direct_ip" | "proxy_sites" | "proxy_ip" | "block_sites" | "block_ip";

const PROFILE_LISTS: { key: ListKey; label: TranslationKey; kind: EntryKind }[] = [
  { key: "direct_sites", label: "routing.directSites", kind: "direct" },
  { key: "direct_ip", label: "routing.directIp", kind: "direct" },
  { key: "proxy_sites", label: "routing.proxySites", kind: "proxy" },
  { key: "proxy_ip", label: "routing.proxyIp", kind: "proxy" },
  { key: "block_sites", label: "routing.blockSites", kind: "block" },
  { key: "block_ip", label: "routing.blockIp", kind: "block" },
];

const CHIP_LIMIT = 12;

function ChipList({ items, kind, onPreview }: { items: string[]; kind?: EntryKind; onPreview?: (entry: string) => void }) {
  const [open, setOpen] = useState(false);
  const shown = open ? items : items.slice(0, CHIP_LIMIT);
  const hidden = items.length - CHIP_LIMIT;
  return (
    <div className="routing-chips">
      {shown.map((item, i) =>
        onPreview && isGeoEntry(item) ? (
          <button
            key={i}
            type="button"
            className={`routing-chip routing-entry clickable ${kind ?? ""}`}
            onClick={() => onPreview(item)}
            title={t("routing.geoPreviewHint")}
          >
            {item}
          </button>
        ) : (
          <span key={i} className={`routing-chip routing-entry ${kind ?? ""}`}>
            {item}
          </span>
        )
      )}
      {hidden > 0 && (
        <button className="routing-more-btn" onClick={() => setOpen(!open)}>
          {open ? t("routing.showLess") : t("routing.showMore").replace("{n}", String(hidden))}
        </button>
      )}
    </div>
  );
}

function ProfileDetails({ profile: p, onPreview }: { profile: RoutingProfile; onPreview: (entry: string) => void }) {
  const hosts = Object.entries(p.dns_hosts ?? {}).map(([host, ip]) => `${host} → ${ip}`);
  const lists = PROFILE_LISTS.filter((l) => (p[l.key]?.length ?? 0) > 0);

  return (
    <div className="routing-profile-details">
      <div className="routing-detail-row">
        <span className="routing-detail-label">{t("routing.routeOrder")}</span>
        <span className="routing-detail-value">{formatRouteOrder(p.route_order)}</span>
      </div>
      <div className="routing-detail-row">
        <span className="routing-detail-label">{t("routing.domainStrategy")}</span>
        <span className="routing-detail-value">{p.domain_strategy || "—"}</span>
      </div>
      <div className="routing-detail-row">
        <span className="routing-detail-label">{t("routing.remoteDns")}</span>
        <span className="routing-detail-value mono">
          {formatDns(p.remote_dns_type, p.remote_dns_domain, p.remote_dns_ip)}
        </span>
      </div>
      <div className="routing-detail-row">
        <span className="routing-detail-label">{t("routing.domesticDns")}</span>
        <span className="routing-detail-value mono">
          {formatDns(p.domestic_dns_type, p.domestic_dns_domain, p.domestic_dns_ip)}
        </span>
      </div>

      {hosts.length > 0 && (
        <div className="routing-detail-group">
          <div className="routing-detail-label">
            {t("routing.dnsHosts")} ({hosts.length})
          </div>
          <ChipList items={hosts} />
        </div>
      )}

      {lists.length === 0 ? (
        <div className="routing-detail-empty">{t("routing.noEntries")}</div>
      ) : (
        lists.map((l) => (
          <div key={l.key} className="routing-detail-group">
            <div className="routing-detail-label">
              {t(l.label)} ({p[l.key].length})
            </div>
            <ChipList items={p[l.key]} kind={l.kind} onPreview={onPreview} />
          </div>
        ))
      )}
    </div>
  );
}

// ── Section ─────────────────────────────────────

export function RoutingProfilesSection({ rp }: { rp: RoutingProfilesApi }) {
  const { state, dispatch, toast } = useApp();
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [geoBusy, setGeoBusy] = useState<Set<string>>(() => new Set());
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [importText, setImportText] = useState("");
  const [importSource, setImportSource] = useState<"field" | "builtin" | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [preview, setPreview] = useState<GeoPreviewTarget | null>(null);
  const [linkShown, setLinkShown] = useState<string | null>(null);
  const editing = rp.profiles.find((p) => p.id === editingId) ?? null;

  const createProfile = async () => {
    setCreating(true);
    try {
      const created = await api.createRoutingProfile(t("routing.newProfileName"));
      await rp.reload();
      setEditingId(created.id);
    } catch (e) {
      toast(errMsg(e), "error");
    } finally {
      setCreating(false);
    }
  };

  const copyLink = async (p: RoutingProfile) => {
    try {
      const link = await api.exportRoutingProfile(p.id);
      try {
        await navigator.clipboard.writeText(link);
        toast(t("routing.linkCopied"), "success");
      } catch {
        setLinkShown(link); // clipboard unavailable: show it for manual copy
      }
    } catch (e) {
      toast(errMsg(e), "error");
    }
  };

  const importBusy = state.routingImportsPending > 0 || importSource !== null;
  const switching = rp.pendingId !== undefined;
  const activeId = rp.activeProfile?.id ?? null;
  const builtinAdded = rp.profiles.some(
    (p) => p.name === BUILTIN_RU_PROFILE.Name && !p.subscription_id
  );

  const doImport = async (input: string, source: "field" | "builtin") => {
    const value = input.trim();
    if (!value || importBusy) return;
    setImportSource(source);
    const ok = await runRoutingImport(value, dispatch, toast);
    setImportSource(null);
    if (ok && source === "field") setImportText("");
  };

  const refreshGeo = async (p: RoutingProfile) => {
    if (geoBusy.has(p.id)) return;
    setGeoBusy((s) => new Set(s).add(p.id));
    try {
      const updated = await api.updateRoutingGeo(p.id);
      rp.replaceProfile(updated);
      toast(`${t("routing.geoUpdatedAt")}: ${updated.name}`, "success");
    } catch (e) {
      toast(errMsg(e), "error");
    } finally {
      setGeoBusy((s) => {
        const next = new Set(s);
        next.delete(p.id);
        return next;
      });
      rp.reload();
    }
  };

  const remove = async (p: RoutingProfile) => {
    let message = `${t("routing.profileDeleteConfirm")} "${p.name}"?`;
    if (p.subscription_id) message += `\n${t("routing.profileDeleteSubNote")}`;
    if (!(await showConfirm(message))) return;
    setDeletingId(p.id);
    try {
      await api.deleteRoutingProfile(p.id);
      if (expandedId === p.id) setExpandedId(null);
      toast(t("routing.profileDeleted"), "info");
    } catch (e) {
      toast(errMsg(e), "error");
    } finally {
      setDeletingId(null);
      rp.reload();
    }
  };

  const radioKeys = (id: string | null) => (e: KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      rp.activate(id);
    }
  };

  const subName = (p: RoutingProfile) =>
    state.subscriptions.find((s) => s.id === p.subscription_id)?.name ?? t("routing.profileFromSub");

  return (
    <div className="settings-section">
      <div className="settings-label">{t("routing.profile")}</div>
      <div className="routing-hint">{t("routing.profileDesc")}</div>

      <div
        className={`routing-profile-list${switching ? " switching" : ""}`}
        role="radiogroup"
        aria-label={t("routing.profile")}
        aria-busy={switching}
      >
        {/* Off */}
        <div className={`routing-profile-card${activeId === null ? " active" : ""}`}>
          <div
            className="routing-profile-main"
            role="radio"
            aria-checked={activeId === null}
            tabIndex={0}
            onClick={() => rp.activate(null)}
            onKeyDown={radioKeys(null)}
          >
            {rp.pendingId === null ? <Spinner size={16} /> : <span className="routing-radio" />}
            <div className="routing-profile-info">
              <span className="routing-profile-name">{t("routing.profileOff")}</span>
              <span className="routing-profile-desc">{t("routing.profileOffDesc")}</span>
            </div>
          </div>
        </div>

        {rp.profiles.map((p) => {
          const active = activeId === p.id;
          const expanded = expandedId === p.id;
          const refreshing = geoBusy.has(p.id);
          const directN = count(p.direct_sites, p.direct_ip);
          const proxyN = count(p.proxy_sites, p.proxy_ip);
          const blockN = count(p.block_sites, p.block_ip);
          return (
            <div
              key={p.id}
              className={`routing-profile-card${active ? " active" : ""}${expanded ? " expanded" : ""}`}
            >
              <div className="routing-profile-top">
                <div
                  className="routing-profile-main"
                  role="radio"
                  aria-checked={active}
                  tabIndex={0}
                  onClick={() => rp.activate(p.id)}
                  onKeyDown={radioKeys(p.id)}
                >
                  {rp.pendingId === p.id ? <Spinner size={16} /> : <span className="routing-radio" />}
                  <div className="routing-profile-info">
                    <div className="routing-profile-name-row">
                      <span className="routing-profile-name" title={p.name}>
                        {p.name}
                      </span>
                      {p.subscription_id && (
                        <span className="routing-sub-badge" title={`${t("routing.profileFromSubTitle")}: ${subName(p)}`}>
                          <FolderIcon size={10} />
                          <span className="routing-sub-badge-text">{subName(p)}</span>
                        </span>
                      )}
                      {p.edited && (
                        <span className="routing-edited-badge" title={t("routing.editedTitle")}>
                          {t("routing.edited")}
                        </span>
                      )}
                    </div>
                    <div className="routing-chips">
                      <span className={`routing-chip direct${directN ? "" : " zero"}`}>
                        {t("routing.catDirect")} <b>{directN}</b>
                      </span>
                      <span className={`routing-chip proxy${proxyN ? "" : " zero"}`}>
                        {t("routing.catProxy")} <b>{proxyN}</b>
                      </span>
                      <span className={`routing-chip block${blockN ? "" : " zero"}`}>
                        {t("routing.catBlock")} <b>{blockN}</b>
                      </span>
                      <span className="routing-chip">
                        {t("routing.otherTraffic")}: {p.global_proxy ? t("routing.viaProxy") : t("routing.viaDirect")}
                      </span>
                    </div>
                    {p.geo_error ? (
                      <div className="routing-profile-geo error">
                        <AlertTriangleIcon size={12} />
                        <span>
                          {t("routing.geoError")}: {p.geo_error}
                        </span>
                      </div>
                    ) : (
                      <div className="routing-profile-geo">
                        {p.geo_updated_at ? <CheckCircleIcon size={12} /> : <DownloadCloudIcon size={12} />}
                        <span>
                          {p.geo_updated_at
                            ? `${t("routing.geoUpdatedAt")}: ${formatTime(p.geo_updated_at)}`
                            : t("routing.geoNever")}
                        </span>
                      </div>
                    )}
                  </div>
                </div>
                <div className="routing-profile-actions">
                  <button
                    className="routing-icon-btn"
                    onClick={() => setEditingId(p.id)}
                    title={t("routing.editProfile")}
                    aria-label={t("routing.editProfile")}
                  >
                    <EditIcon size={15} />
                  </button>
                  <button
                    className="routing-icon-btn"
                    onClick={() => copyLink(p)}
                    title={t("routing.copyLink")}
                    aria-label={t("routing.copyLink")}
                  >
                    <CopyIcon size={15} />
                  </button>
                  <button
                    className="routing-icon-btn"
                    onClick={() => setExpandedId(expanded ? null : p.id)}
                    title={expanded ? t("routing.hideDetails") : t("routing.showDetails")}
                    aria-label={expanded ? t("routing.hideDetails") : t("routing.showDetails")}
                    aria-expanded={expanded}
                  >
                    <ChevronDownIcon size={16} className={`routing-chevron${expanded ? " open" : ""}`} />
                  </button>
                  <button
                    className="routing-icon-btn"
                    onClick={() => refreshGeo(p)}
                    disabled={refreshing}
                    title={t("routing.geoRefresh")}
                    aria-label={t("routing.geoRefresh")}
                  >
                    {refreshing ? <Spinner size={14} /> : <RefreshCwIcon size={15} />}
                  </button>
                  <button
                    className="routing-icon-btn danger"
                    onClick={() => remove(p)}
                    disabled={deletingId === p.id}
                    title={t("routing.deleteProfile")}
                    aria-label={t("routing.deleteProfile")}
                  >
                    {deletingId === p.id ? <Spinner size={14} /> : <TrashIcon size={15} />}
                  </button>
                </div>
              </div>
              {expanded && <ProfileDetails profile={p} onPreview={(entry) => setPreview({ profileId: p.id, entry })} />}
            </div>
          );
        })}

        {!rp.loaded && (
          <div className="routing-profile-loading">
            <Spinner size={16} />
          </div>
        )}
        {rp.loaded && rp.loadError && (
          <div className="routing-profile-geo error">
            <AlertTriangleIcon size={12} />
            <span>
              {t("routing.profilesLoadError")}: {rp.loadError}
            </span>
          </div>
        )}
      </div>

      <div className="routing-divider" />

      <div className="settings-label">{t("routing.addProfile")}</div>
      <div className="routing-profile-import">
        <input
          className="form-input"
          type="text"
          placeholder={t("routing.importPlaceholder")}
          value={importText}
          onChange={(e) => setImportText(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && doImport(importText, "field")}
          disabled={importBusy}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          autoComplete="off"
        />
        <Button
          size="sm"
          onClick={() => doImport(importText, "field")}
          disabled={importBusy || !importText.trim()}
        >
          {importBusy && importSource !== "builtin" ? <Spinner size={14} /> : t("common.import")}
        </Button>
      </div>
      {importBusy && (
        <div className="routing-busy-hint">
          <Spinner size={14} />
          <span>{t("routing.importBusy")}</span>
        </div>
      )}

      <div className="routing-builtin">
        <Button
          variant="secondary"
          size="sm"
          onClick={() => doImport(JSON.stringify(BUILTIN_RU_PROFILE), "builtin")}
          disabled={importBusy || builtinAdded}
          title={builtinAdded ? t("routing.builtinAdded") : t("routing.builtinRuDesc")}
        >
          {importSource === "builtin" ? (
            <Spinner size={14} />
          ) : builtinAdded ? (
            <CheckCircleIcon size={14} />
          ) : (
            <DownloadCloudIcon size={14} />
          )}
          <span className="routing-builtin-label">{t("routing.builtinRu")}</span>
        </Button>
        <span className="routing-builtin-desc">
          {builtinAdded ? t("routing.builtinAdded") : t("routing.builtinRuDesc")}
        </span>
      </div>

      <div className="routing-builtin">
        <Button variant="secondary" size="sm" onClick={createProfile} disabled={creating}>
          {creating ? <Spinner size={14} /> : <PlusIcon size={14} />}
          <span className="routing-builtin-label">{t("routing.createProfile")}</span>
        </Button>
        <span className="routing-builtin-desc">{t("routing.createProfileDesc")}</span>
      </div>

      <RoutingProfileEditor
        profile={editing}
        subscriptionName={editing?.subscription_id ? subName(editing) : null}
        onClose={() => setEditingId(null)}
        onSaved={(saved) => {
          rp.replaceProfile(saved);
          setEditingId(null);
          rp.reload();
        }}
        onReload={rp.reload}
      />
      <GeoPreviewModal target={preview} onClose={() => setPreview(null)} />
      <Modal open={linkShown !== null} onClose={() => setLinkShown(null)} title={t("routing.linkTitle")}>
        <textarea
          className="form-input mono routing-link-box"
          readOnly
          value={linkShown ?? ""}
          onFocus={(e) => e.currentTarget.select()}
          rows={5}
        />
      </Modal>
    </div>
  );
}
