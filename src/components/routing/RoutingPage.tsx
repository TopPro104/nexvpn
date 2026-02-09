import { useState, useCallback, useRef } from "react";
import { useApp } from "../../context/AppContext";
import { api, RoutingRule, RuleAction } from "../../api/tauri";
import { t } from "../../i18n/translations";

const ADS_DOMAINS = [
  "doubleclick.net",
  "googlesyndication.com",
  "googleadservices.com",
  "adnxs.com",
  "ads.yahoo.com",
  "moatads.com",
  "adcolony.com",
];

const RU_DOMAINS = [
  "yandex.ru",
  "mail.ru",
  "vk.com",
  "ok.ru",
  "dzen.ru",
  "rutube.ru",
  "gosuslugi.ru",
];

let idCounter = Date.now();
function nextId() {
  return String(++idCounter);
}

export function RoutingPage() {
  const { state, dispatch, toast } = useApp();
  void state.langTick;

  const [newDomain, setNewDomain] = useState("");
  const [newAction, setNewAction] = useState<RuleAction>("direct");
  const saveTimer = useRef<ReturnType<typeof setTimeout>>();

  const save = useCallback(
    (rules: RoutingRule[], defaultRoute: string) => {
      dispatch({ type: "SET_ROUTING_RULES", rules, defaultRoute });
      clearTimeout(saveTimer.current);
      saveTimer.current = setTimeout(async () => {
        try {
          await api.saveRoutingRules(rules, defaultRoute);
          toast(t("routing.saved"), "success");
        } catch (e) {
          toast(`${e}`, "error");
        }
      }, 800);
    },
    [dispatch, toast]
  );

  const setDefaultRoute = (route: string) => {
    save(state.routingRules, route);
  };

  const addRule = () => {
    const domain = newDomain.trim().toLowerCase();
    if (!domain) return;
    if (state.routingRules.some((r) => r.domain === domain)) {
      toast(`Rule for ${domain} already exists`, "error");
      return;
    }
    const rule: RoutingRule = {
      id: nextId(),
      domain,
      action: newAction,
      enabled: true,
    };
    save([...state.routingRules, rule], state.defaultRoute);
    setNewDomain("");
  };

  const removeRule = (id: string) => {
    save(
      state.routingRules.filter((r) => r.id !== id),
      state.defaultRoute
    );
  };

  const toggleRule = (id: string) => {
    save(
      state.routingRules.map((r) =>
        r.id === id ? { ...r, enabled: !r.enabled } : r
      ),
      state.defaultRoute
    );
  };

  const addPreset = (domains: string[], action: RuleAction) => {
    const existing = new Set(state.routingRules.map((r) => r.domain));
    const newRules = domains
      .filter((d) => !existing.has(d))
      .map((domain) => ({
        id: nextId(),
        domain,
        action,
        enabled: true,
      }));
    if (newRules.length === 0) {
      toast("All preset domains already added", "info");
      return;
    }
    save([...state.routingRules, ...newRules], state.defaultRoute);
  };

  const actionColor = (action: RuleAction) => {
    switch (action) {
      case "proxy":
        return "var(--accent)";
      case "direct":
        return "var(--success)";
      case "block":
        return "var(--danger)";
    }
  };

  return (
    <div className="routing-page">
      <h2>{t("routing.title")}</h2>

      {/* Default route toggle */}
      <div className="settings-section">
        <div className="settings-label">{t("routing.defaultRoute")}</div>
        <div className="vpn-mode-group">
          <label
            className={`vpn-mode-card ${state.defaultRoute === "proxy" ? "active" : ""}`}
            onClick={() => setDefaultRoute("proxy")}
          >
            <input
              type="radio"
              name="defaultRoute"
              checked={state.defaultRoute === "proxy"}
              onChange={() => setDefaultRoute("proxy")}
            />
            <div className="vpn-mode-info">
              <span className="vpn-mode-title">{t("routing.proxyAll")}</span>
              <span className="vpn-mode-desc">{t("routing.proxyAllDesc")}</span>
            </div>
          </label>
          <label
            className={`vpn-mode-card ${state.defaultRoute === "direct" ? "active" : ""}`}
            onClick={() => setDefaultRoute("direct")}
          >
            <input
              type="radio"
              name="defaultRoute"
              checked={state.defaultRoute === "direct"}
              onChange={() => setDefaultRoute("direct")}
            />
            <div className="vpn-mode-info">
              <span className="vpn-mode-title">{t("routing.directAll")}</span>
              <span className="vpn-mode-desc">{t("routing.directAllDesc")}</span>
            </div>
          </label>
        </div>
      </div>

      {/* Add rule form */}
      <div className="settings-section">
        <div className="settings-label">{t("routing.addRule")}</div>
        <div className="routing-add-form">
          <input
            className="form-input routing-domain-input"
            type="text"
            placeholder={t("routing.domainPlaceholder")}
            value={newDomain}
            onChange={(e) => setNewDomain(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && addRule()}
          />
          <select
            className="sort-select routing-action-select"
            value={newAction}
            onChange={(e) => setNewAction(e.target.value as RuleAction)}
          >
            <option value="direct">{t("routing.direct")}</option>
            <option value="proxy">{t("routing.proxy")}</option>
            <option value="block">{t("routing.block")}</option>
          </select>
          <button className="btn btn-primary btn-sm" onClick={addRule}>
            {t("common.add")}
          </button>
        </div>
      </div>

      {/* Presets */}
      <div className="settings-section">
        <div className="settings-label">{t("routing.presets")}</div>
        <div className="routing-presets">
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => addPreset(ADS_DOMAINS, "block")}
          >
            {t("routing.presetAds")}
          </button>
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => addPreset(RU_DOMAINS, "direct")}
          >
            {t("routing.presetRuDirect")}
          </button>
        </div>
      </div>

      {/* Rules list */}
      <div className="settings-section">
        <div className="settings-label">
          {t("routing.rules")} ({state.routingRules.length})
        </div>
        {state.routingRules.length === 0 ? (
          <div className="empty-list">{t("routing.noRules")}</div>
        ) : (
          <div className="routing-rules-list">
            {state.routingRules.map((rule) => (
              <div
                key={rule.id}
                className={`routing-rule-card ${!rule.enabled ? "disabled" : ""}`}
              >
                <span
                  className="routing-action-badge"
                  style={{
                    background: `color-mix(in srgb, ${actionColor(rule.action)} 15%, transparent)`,
                    color: actionColor(rule.action),
                    borderColor: `color-mix(in srgb, ${actionColor(rule.action)} 30%, transparent)`,
                  }}
                >
                  {rule.action}
                </span>
                <span className="routing-rule-domain">{rule.domain}</span>
                <label className="toggle routing-rule-toggle">
                  <input
                    type="checkbox"
                    checked={rule.enabled}
                    onChange={() => toggleRule(rule.id)}
                  />
                  <span className="toggle-slider" />
                </label>
                <button
                  className="routing-rule-delete"
                  onClick={() => removeRule(rule.id)}
                >
                  x
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
