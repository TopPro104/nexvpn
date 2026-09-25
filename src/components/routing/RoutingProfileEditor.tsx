import { useState, useEffect, useMemo, useRef, ClipboardEvent, KeyboardEvent } from "react";
import { useApp } from "../../context/AppContext";
import { api, GeoCategory, GeoCodes, RoutingProfile } from "../../api/tauri";
import { t, TranslationKey } from "../../i18n/translations";
import { showConfirm } from "../../utils/confirm";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";
import { Spinner } from "../ui/Spinner";
import { AlertTriangleIcon, InfoIcon, PlusIcon, RefreshCwIcon, RotateCcwIcon, XIcon } from "../ui/Icons";

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e));

export const isGeoEntry = (entry: string) => /^geo(site|ip):/i.test(entry.trim());

// ── Geo category preview ────────────────────────

export interface GeoPreviewTarget {
  profileId: string | null; // null = geo files used by custom rules
  entry: string;
}

/** Shows what a geosite:/geoip: entry contains in the downloaded geo files. */
export function GeoPreviewModal({ target, onClose }: { target: GeoPreviewTarget | null; onClose: () => void }) {
  const [data, setData] = useState<GeoCategory | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setData(null);
    setError(null);
    if (!target) return;
    let cancelled = false;
    api
      .getGeoCategory(target.profileId, target.entry)
      .then((d) => !cancelled && setData(d))
      .catch((e) => !cancelled && setError(errMsg(e)));
    return () => {
      cancelled = true;
    };
  }, [target]);

  return (
    <Modal open={target !== null} onClose={onClose} title={target?.entry ?? ""} className="routing-geo-modal">
      {!data && !error && (
        <div className="routing-geo-loading">
          <Spinner size={18} />
        </div>
      )}
      {error && (
        <div className="routing-profile-geo error">
          <AlertTriangleIcon size={12} />
          <span>{error}</span>
        </div>
      )}
      {data && !data.found && (
        <div className="routing-profile-geo error">
          <AlertTriangleIcon size={12} />
          <span>{t("routing.geoPreviewNotFound")}</span>
        </div>
      )}
      {data && data.found && (
        <>
          <div className="routing-geo-meta">
            <span>{t("routing.geoPreviewTotal").replace("{n}", String(data.total))}</span>
            {data.total > data.items.length && (
              <span>{t("routing.geoPreviewFirst").replace("{n}", String(data.items.length))}</span>
            )}
          </div>
          <div className="routing-geo-items">
            {data.items.map((item, i) => (
              <div key={i} className="routing-geo-item">
                {item}
              </div>
            ))}
          </div>
        </>
      )}
    </Modal>
  );
}

// ── Chip editor ─────────────────────────────────

function splitEntries(text: string): string[] {
  return text
    .split(/[\s,;]+/)
    .map((s) => s.trim())
    .filter(Boolean);
}

interface ChipEditorProps {
  values: string[];
  onChange: (values: string[]) => void;
  kind: "direct" | "proxy" | "block";
  suggestions: string[];
  listId: string;
  placeholder: string;
  onPreview: (entry: string) => void;
}

function ChipEditor({ values, onChange, kind, suggestions, listId, placeholder, onPreview }: ChipEditorProps) {
  const [input, setInput] = useState("");

  const add = (entries: string[]) => {
    const next = [...values];
    for (const e of entries) if (!next.includes(e)) next.push(e);
    if (next.length !== values.length) onChange(next);
    setInput("");
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      add(splitEntries(input));
    } else if (e.key === "Backspace" && !input && values.length > 0) {
      onChange(values.slice(0, -1));
    }
  };

  const onPaste = (e: ClipboardEvent<HTMLInputElement>) => {
    const text = e.clipboardData.getData("text");
    const entries = splitEntries(text);
    if (entries.length > 1) {
      e.preventDefault();
      add(entries);
    }
  };

  return (
    <div className="routing-chip-editor">
      {values.length > 0 && (
        <div className="routing-chips">
          {values.map((v) => (
            <span key={v} className={`routing-chip routing-entry editable ${kind}`}>
              {isGeoEntry(v) ? (
                <button type="button" className="routing-chip-link" onClick={() => onPreview(v)} title={t("routing.geoPreviewHint")}>
                  {v}
                </button>
              ) : (
                <span>{v}</span>
              )}
              <button
                type="button"
                className="routing-chip-remove"
                onClick={() => onChange(values.filter((x) => x !== v))}
                aria-label={`${t("common.delete")} ${v}`}
              >
                <XIcon size={11} />
              </button>
            </span>
          ))}
        </div>
      )}
      <div className="routing-chip-input-row">
        <input
          className="form-input"
          value={input}
          list={listId}
          placeholder={placeholder}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={onKeyDown}
          onPaste={onPaste}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          autoComplete="off"
        />
        <button
          type="button"
          className="routing-icon-btn"
          onClick={() => add(splitEntries(input))}
          disabled={!input.trim()}
          aria-label={t("common.add")}
          title={t("common.add")}
        >
          <PlusIcon size={15} />
        </button>
      </div>
      <datalist id={listId}>
        {suggestions.map((s) => (
          <option key={s} value={s} />
        ))}
      </datalist>
    </div>
  );
}

// ── Profile editor ──────────────────────────────

const ROUTE_ORDERS = [
  "block-proxy-direct",
  "block-direct-proxy",
  "proxy-direct-block",
  "proxy-block-direct",
  "direct-proxy-block",
  "direct-block-proxy",
];

const ORDER_PART: Record<string, TranslationKey> = {
  block: "routing.catBlock",
  proxy: "routing.catProxy",
  direct: "routing.catDirect",
};

const orderLabel = (order: string) =>
  order
    .split("-")
    .map((p) => (ORDER_PART[p] ? t(ORDER_PART[p]) : p))
    .join(" → ");

const STRATEGIES: { value: string; hint: TranslationKey }[] = [
  { value: "IPIfNonMatch", hint: "routing.strategyIPIfNonMatch" },
  { value: "AsIs", hint: "routing.strategyAsIs" },
  { value: "IPOnDemand", hint: "routing.strategyIPOnDemand" },
];

type ListKey = "direct_sites" | "direct_ip" | "proxy_sites" | "proxy_ip" | "block_sites" | "block_ip";

const RULE_GROUPS: { kind: "direct" | "proxy" | "block"; title: TranslationKey; sites: ListKey; ips: ListKey }[] = [
  { kind: "direct", title: "routing.catDirect", sites: "direct_sites", ips: "direct_ip" },
  { kind: "proxy", title: "routing.catProxy", sites: "proxy_sites", ips: "proxy_ip" },
  { kind: "block", title: "routing.catBlock", sites: "block_sites", ips: "block_ip" },
];

type HostRow = { host: string; ip: string };

const hostsToRows = (hosts: Record<string, string>): HostRow[] =>
  Object.entries(hosts ?? {}).map(([host, ip]) => ({ host, ip }));

const rowsToHosts = (rows: HostRow[]): Record<string, string> => {
  const out: Record<string, string> = {};
  for (const r of rows) if (r.host.trim() || r.ip.trim()) out[r.host.trim()] = r.ip.trim();
  return out;
};

/** Content that the user can edit, for dirty checks */
const editableSnapshot = (p: RoutingProfile, rows: HostRow[]) =>
  JSON.stringify({
    ...p,
    dns_hosts: rowsToHosts(rows),
    geo_updated_at: null,
    geo_error: null,
    original: null,
    edited: false,
  });

interface EditorProps {
  profile: RoutingProfile | null;
  subscriptionName: string | null;
  onClose: () => void;
  onSaved: (profile: RoutingProfile) => void;
  /** Called after a failed save, so the list can reload (the profile may have been saved) */
  onReload: () => void;
}

export function RoutingProfileEditor({ profile, subscriptionName, onClose, onSaved, onReload }: EditorProps) {
  const { toast } = useApp();
  const [draft, setDraft] = useState<RoutingProfile | null>(profile);
  const [hostRows, setHostRows] = useState<HostRow[]>(() => hostsToRows(profile?.dns_hosts ?? {}));
  const [codes, setCodes] = useState<GeoCodes | null>(null);
  const [saving, setSaving] = useState(false);
  const [resetting, setResetting] = useState(false);
  const [geoBusy, setGeoBusy] = useState(false);
  const [preview, setPreview] = useState<GeoPreviewTarget | null>(null);
  const initial = useRef<string>("");

  useEffect(() => {
    setDraft(profile);
    const rows = hostsToRows(profile?.dns_hosts ?? {});
    setHostRows(rows);
    initial.current = profile ? editableSnapshot(profile, rows) : "";
    setCodes(null);
    if (!profile) return;
    let cancelled = false;
    api
      .getGeoCodes(profile.id)
      .then((c) => !cancelled && setCodes(c))
      .catch(() => !cancelled && setCodes({ geosite: [], geoip: [] }));
    return () => {
      cancelled = true;
    };
    // Re-initialise only when a different profile is opened
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [profile?.id]);

  const siteSuggestions = useMemo(() => (codes?.geosite ?? []).map((c) => `geosite:${c}`), [codes]);
  const ipSuggestions = useMemo(() => (codes?.geoip ?? []).map((c) => `geoip:${c}`), [codes]);

  if (!profile || !draft) return null;

  const dirty = editableSnapshot(draft, hostRows) !== initial.current;
  const busy = saving || resetting;
  const set = <K extends keyof RoutingProfile>(key: K, value: RoutingProfile[K]) =>
    setDraft((d) => (d ? { ...d, [key]: value } : d));

  const requestClose = async () => {
    if (busy) return;
    if (preview) return; // Escape closes the preview first
    if (dirty && !(await showConfirm(t("routing.discardChanges")))) return;
    onClose();
  };

  const save = async () => {
    setSaving(true);
    try {
      const saved = await api.saveRoutingProfile({ ...draft, dns_hosts: rowsToHosts(hostRows) });
      toast(`${t("routing.profileSaved")}: ${saved.name}`, "success");
      if (saved.geo_error) toast(`${t("routing.geoError")}: ${saved.geo_error}`, "error");
      onSaved(saved);
    } catch (e) {
      toast(errMsg(e), "error");
      onReload();
    } finally {
      setSaving(false);
    }
  };

  const reset = async () => {
    if (!(await showConfirm(t("routing.resetConfirm")))) return;
    setResetting(true);
    try {
      const restored = await api.resetRoutingProfile(profile.id);
      toast(t("routing.profileReset"), "success");
      onSaved(restored);
    } catch (e) {
      toast(errMsg(e), "error");
      onReload();
    } finally {
      setResetting(false);
    }
  };

  const refreshGeo = async () => {
    setGeoBusy(true);
    try {
      const updated = await api.updateRoutingGeo(profile.id);
      setCodes(await api.getGeoCodes(profile.id));
      set("geo_updated_at", updated.geo_updated_at);
      set("geo_error", updated.geo_error);
      toast(`${t("routing.geoUpdatedAt")}: ${updated.name}`, "success");
    } catch (e) {
      toast(errMsg(e), "error");
    } finally {
      setGeoBusy(false);
      onReload();
    }
  };

  const dnsBlock = (
    title: TranslationKey,
    desc: TranslationKey,
    typeKey: "remote_dns_type" | "domestic_dns_type",
    urlKey: "remote_dns_domain" | "domestic_dns_domain",
    ipKey: "remote_dns_ip" | "domestic_dns_ip"
  ) => (
    <div className="routing-editor-card">
      <div className="routing-editor-card-title">{t(title)}</div>
      <div className="routing-hint">{t(desc)}</div>
      <div className="routing-editor-grid">
        <label className="routing-field">
          <span>{t("routing.dnsType")}</span>
          <select className="sort-select" value={draft[typeKey]} onChange={(e) => set(typeKey, e.target.value)}>
            <option value="DoH">DoH (DNS over HTTPS)</option>
            <option value="DoU">DoU (DNS over UDP)</option>
          </select>
        </label>
        <label className="routing-field">
          <span>{t("routing.dnsIp")}</span>
          <input className="form-input mono" value={draft[ipKey]} onChange={(e) => set(ipKey, e.target.value)} placeholder="1.1.1.1" spellCheck={false} />
        </label>
        {draft[typeKey] === "DoH" && (
          <label className="routing-field wide">
            <span>{t("routing.dnsUrl")}</span>
            <input
              className="form-input mono"
              value={draft[urlKey]}
              onChange={(e) => set(urlKey, e.target.value)}
              placeholder="https://1.1.1.1/dns-query"
              spellCheck={false}
            />
          </label>
        )}
      </div>
    </div>
  );

  const strategyHint = STRATEGIES.find((s) => s.value === draft.domain_strategy)?.hint;

  return (
    <>
      <Modal open onClose={requestClose} title={`${t("routing.editProfile")}: ${profile.name}`} className="routing-editor-modal">
        <div className="routing-editor">
          {profile.subscription_id && (
            <div className="routing-lock-note">
              <InfoIcon size={14} />
              <span>
                {subscriptionName ? `${subscriptionName}: ` : ""}
                {t("routing.subEditNote")}
              </span>
            </div>
          )}

          {/* General */}
          <div className="routing-editor-section">
            <div className="settings-label">{t("routing.secGeneral")}</div>
            <div className="routing-editor-grid">
              <label className="routing-field wide">
                <span>{t("routing.name")}</span>
                <input className="form-input" value={draft.name} onChange={(e) => set("name", e.target.value)} />
              </label>
              <div className="routing-field wide">
                <span>{t("routing.otherTraffic")}</span>
                <div className="routing-seg" role="radiogroup">
                  <button
                    type="button"
                    role="radio"
                    aria-checked={draft.global_proxy}
                    className={draft.global_proxy ? "active" : ""}
                    onClick={() => set("global_proxy", true)}
                  >
                    {t("routing.viaProxy")}
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={!draft.global_proxy}
                    className={!draft.global_proxy ? "active" : ""}
                    onClick={() => set("global_proxy", false)}
                  >
                    {t("routing.viaDirect")}
                  </button>
                </div>
              </div>
              <label className="routing-field">
                <span>{t("routing.routeOrder")}</span>
                <select className="sort-select" value={draft.route_order} onChange={(e) => set("route_order", e.target.value)}>
                  {ROUTE_ORDERS.map((o) => (
                    <option key={o} value={o}>
                      {orderLabel(o)}
                    </option>
                  ))}
                </select>
              </label>
              <label className="routing-field">
                <span>{t("routing.domainStrategy")}</span>
                <select className="sort-select" value={draft.domain_strategy} onChange={(e) => set("domain_strategy", e.target.value)}>
                  {STRATEGIES.map((s) => (
                    <option key={s.value} value={s.value}>
                      {s.value}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            {strategyHint && <div className="routing-hint">{t(strategyHint)}</div>}
          </div>

          {/* Rules */}
          <div className="routing-editor-section">
            <div className="settings-label">{t("routing.secRules")}</div>
            <div className="routing-hint">{t("routing.profileSyntaxHint")}</div>
            {codes && (
              <div className="routing-hint">
                {codes.geosite.length || codes.geoip.length
                  ? t("routing.geoCodesHint").replace("{n}", String(codes.geosite.length)).replace("{m}", String(codes.geoip.length))
                  : t("routing.geoCodesMissing")}
              </div>
            )}
            {RULE_GROUPS.map((g) => (
              <div key={g.kind} className={`routing-editor-card rules ${g.kind}`}>
                <div className={`routing-editor-card-title ${g.kind}`}>{t(g.title)}</div>
                <div className="routing-field">
                  <span>{t("routing.sites")}</span>
                  <ChipEditor
                    values={draft[g.sites]}
                    onChange={(v) => set(g.sites, v)}
                    kind={g.kind}
                    suggestions={siteSuggestions}
                    listId="routing-geosite-codes"
                    placeholder={t("routing.entryPlaceholderSite")}
                    onPreview={(entry) => setPreview({ profileId: profile.id, entry })}
                  />
                </div>
                <div className="routing-field">
                  <span>{t("routing.ips")}</span>
                  <ChipEditor
                    values={draft[g.ips]}
                    onChange={(v) => set(g.ips, v)}
                    kind={g.kind}
                    suggestions={ipSuggestions}
                    listId="routing-geoip-codes"
                    placeholder={t("routing.entryPlaceholderIp")}
                    onPreview={(entry) => setPreview({ profileId: profile.id, entry })}
                  />
                </div>
              </div>
            ))}
          </div>

          {/* DNS */}
          <div className="routing-editor-section">
            <div className="settings-label">{t("routing.secDns")}</div>
            {dnsBlock("routing.dnsRemoteTitle", "routing.dnsRemoteDesc", "remote_dns_type", "remote_dns_domain", "remote_dns_ip")}
            {dnsBlock("routing.dnsDomesticTitle", "routing.dnsDomesticDesc", "domestic_dns_type", "domestic_dns_domain", "domestic_dns_ip")}
            <div className="routing-editor-card">
              <div className="routing-editor-card-title">{t("routing.dnsHosts")}</div>
              {hostRows.length === 0 && <div className="routing-hint">{t("routing.noHosts")}</div>}
              {hostRows.map((row, i) => (
                <div key={i} className="routing-host-row">
                  <input
                    className="form-input mono"
                    value={row.host}
                    placeholder={t("routing.hostName")}
                    onChange={(e) => setHostRows((rows) => rows.map((r, j) => (j === i ? { ...r, host: e.target.value } : r)))}
                    spellCheck={false}
                  />
                  <input
                    className="form-input mono"
                    value={row.ip}
                    placeholder="IP"
                    onChange={(e) => setHostRows((rows) => rows.map((r, j) => (j === i ? { ...r, ip: e.target.value } : r)))}
                    spellCheck={false}
                  />
                  <button
                    type="button"
                    className="routing-icon-btn danger"
                    onClick={() => setHostRows((rows) => rows.filter((_, j) => j !== i))}
                    aria-label={t("common.delete")}
                  >
                    <XIcon size={14} />
                  </button>
                </div>
              ))}
              <Button variant="secondary" size="sm" onClick={() => setHostRows((rows) => [...rows, { host: "", ip: "" }])}>
                <PlusIcon size={14} />
                <span>{t("routing.addHost")}</span>
              </Button>
            </div>
          </div>

          {/* Geo files */}
          <div className="routing-editor-section">
            <div className="settings-label">{t("routing.secGeo")}</div>
            <div className="routing-editor-grid">
              <label className="routing-field wide">
                <span>{t("routing.geositeUrl")}</span>
                <input className="form-input mono" value={draft.geosite_url} onChange={(e) => set("geosite_url", e.target.value)} spellCheck={false} />
              </label>
              <label className="routing-field wide">
                <span>{t("routing.geoipUrl")}</span>
                <input className="form-input mono" value={draft.geoip_url} onChange={(e) => set("geoip_url", e.target.value)} spellCheck={false} />
              </label>
            </div>
            <div className="routing-hint">{t("routing.geoUrlNote")}</div>
            <div className="routing-geo-row">
              {draft.geo_error ? (
                <div className="routing-profile-geo error">
                  <AlertTriangleIcon size={12} />
                  <span>
                    {t("routing.geoError")}: {draft.geo_error}
                  </span>
                </div>
              ) : (
                <div className="routing-profile-geo">
                  <span>
                    {draft.geo_updated_at
                      ? `${t("routing.geoUpdatedAt")}: ${new Date(draft.geo_updated_at * 1000).toLocaleString()}`
                      : t("routing.geoNever")}
                  </span>
                </div>
              )}
              <Button variant="secondary" size="sm" onClick={refreshGeo} disabled={geoBusy}>
                {geoBusy ? <Spinner size={14} /> : <RefreshCwIcon size={14} />}
                <span>{t("routing.geoRefresh")}</span>
              </Button>
            </div>
          </div>
        </div>

        <div className="routing-editor-footer">
          {saving && (
            <div className="routing-busy-hint">
              <Spinner size={14} />
              <span>{t("routing.saving")}</span>
            </div>
          )}
          <div className="routing-editor-buttons">
            {profile.edited && (
              <Button variant="ghost" size="sm" onClick={reset} disabled={busy} title={t("routing.resetToOriginal")}>
                {resetting ? <Spinner size={14} /> : <RotateCcwIcon size={14} />}
                <span>{t("routing.resetToOriginal")}</span>
              </Button>
            )}
            <span className="routing-editor-spacer" />
            <Button variant="secondary" size="sm" onClick={requestClose} disabled={busy}>
              {t("common.cancel")}
            </Button>
            <Button size="sm" onClick={save} disabled={busy || !dirty}>
              {saving ? <Spinner size={14} /> : t("routing.save")}
            </Button>
          </div>
        </div>
      </Modal>
      <GeoPreviewModal target={preview} onClose={() => setPreview(null)} />
    </>
  );
}
