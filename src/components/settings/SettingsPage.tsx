import { useRef, useCallback, useEffect } from "react";
import { useApp } from "../../context/AppContext";
import { api, Settings, DeviceInfo } from "../../api/tauri";
import { ThemePicker } from "./ThemePicker";
import { t, setLang, Lang } from "../../i18n/translations";
import { useState } from "react";

const styles = [
  { id: "default", label: "Default" },
  { id: "minimal", label: "Modern Minimal" },
  { id: "glass", label: "Glassmorphism" },
  { id: "neon", label: "Neon Glow" },
];

const animations = [
  { id: "none", labelKey: "settings.animation.none" as const },
  { id: "smooth", labelKey: "settings.animation.smooth" as const },
  { id: "energetic", labelKey: "settings.animation.energetic" as const },
];

export function SettingsPage() {
  const { state, dispatch, toast } = useApp();
  void state.langTick;
  const [deviceInfo, setDeviceInfo] = useState<DeviceInfo | null>(null);
  const [hwidCopied, setHwidCopied] = useState(false);
  const [isAdmin, setIsAdmin] = useState(true); // assume admin until checked
  const saveTimer = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    api.getDeviceInfo().then(setDeviceInfo).catch(() => {});
    api.isAdmin().then(setIsAdmin).catch(() => {});
  }, []);

  // Auto-apply: save settings with debounce
  const autoSave = useCallback(
    (newSettings: Settings) => {
      // Update language synchronously BEFORE dispatch so t() returns correct
      // values during the re-render (useEffect runs too late)
      setLang(newSettings.language as Lang);
      dispatch({ type: "SET_SETTINGS", settings: newSettings });
      dispatch({ type: "BUMP_LANG" });

      clearTimeout(saveTimer.current);
      saveTimer.current = setTimeout(async () => {
        // Validate ports
        if (
          newSettings.socks_port < 1 || newSettings.socks_port > 65535 ||
          newSettings.http_port < 1 || newSettings.http_port > 65535
        ) {
          return;
        }
        if (newSettings.socks_port === newSettings.http_port) {
          return;
        }
        try {
          await api.saveSettings(newSettings);
        } catch (e) {
          toast(`${e}`, "error");
        }
      }, 600);
    },
    [dispatch, toast]
  );

  const update = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    autoSave({ ...state.settings, [key]: value });
  };

  const handleCoreChange = async (core: string) => {
    try {
      await api.setCoreType(core);
      const status = await api.getStatus();
      dispatch({ type: "SET_STATUS", status });
      toast(`Core: ${status.core_type}`, "success");
    } catch (e) {
      toast(`${e}`, "error");
    }
  };

  return (
    <div className="settings-page">
      <h2>{t("settings.title")}</h2>

      <div className="settings-section">
        <div className="settings-label">{t("settings.theme")}</div>
        <ThemePicker
          current={state.settings.theme}
          onChange={(name) => update("theme", name)}
        />
      </div>

      <div className="settings-section">
        <div className="settings-label">{t("settings.style")}</div>
        <div className="core-radio-group">
          {styles.map((s) => (
            <label
              key={s.id}
              className={`core-radio ${state.settings.style === s.id ? "active" : ""}`}
            >
              <input
                type="radio"
                name="style"
                checked={state.settings.style === s.id}
                onChange={() => update("style", s.id)}
              />
              <span>{s.label}</span>
            </label>
          ))}
        </div>
      </div>

      <div className="settings-section">
        <div className="settings-label">{t("settings.animation")}</div>
        <div className="core-radio-group">
          {animations.map((a) => (
            <label
              key={a.id}
              className={`core-radio ${state.settings.animation === a.id ? "active" : ""}`}
            >
              <input
                type="radio"
                name="animation"
                checked={state.settings.animation === a.id}
                onChange={() => update("animation", a.id)}
              />
              <span>{t(a.labelKey)}</span>
            </label>
          ))}
        </div>
      </div>

      <div className="settings-section">
        <div className="settings-label">{t("settings.core")}</div>
        <div className="core-radio-group">
          <label className={`core-radio ${state.coreType === "SingBox" ? "active" : ""}`}>
            <input
              type="radio"
              name="core"
              checked={state.coreType === "SingBox"}
              onChange={() => handleCoreChange("singbox")}
            />
            <span>sing-box</span>
          </label>
          <label className={`core-radio ${state.coreType === "Xray" ? "active" : ""}`}>
            <input
              type="radio"
              name="core"
              checked={state.coreType === "Xray"}
              onChange={() => handleCoreChange("xray")}
            />
            <span>Xray-core</span>
          </label>
        </div>
      </div>

      <div className="settings-section">
        <div className="settings-label">{t("settings.vpnMode")}</div>
        <div className="vpn-mode-group">
          <label
            className={`vpn-mode-card ${state.settings.vpn_mode === "proxy" ? "active" : ""}`}
            onClick={() => update("vpn_mode", "proxy")}
          >
            <input
              type="radio"
              name="vpnMode"
              checked={state.settings.vpn_mode === "proxy"}
              onChange={() => update("vpn_mode", "proxy")}
            />
            <div className="vpn-mode-info">
              <span className="vpn-mode-title">{t("settings.vpnMode.proxy")}</span>
              <span className="vpn-mode-desc">{t("settings.vpnMode.proxyDesc")}</span>
            </div>
          </label>
          <label
            className={`vpn-mode-card ${state.settings.vpn_mode === "tun" ? "active" : ""}`}
            onClick={() => update("vpn_mode", "tun")}
          >
            <input
              type="radio"
              name="vpnMode"
              checked={state.settings.vpn_mode === "tun"}
              onChange={() => update("vpn_mode", "tun")}
            />
            <div className="vpn-mode-info">
              <span className="vpn-mode-title">{t("settings.vpnMode.tun")}</span>
              <span className="vpn-mode-desc">{t("settings.vpnMode.tunDesc")}</span>
              {!isAdmin && (
                <button
                  className="btn btn-elevate btn-sm"
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    api.restartAsAdmin().catch((err) => toast(`${err}`, "error"));
                  }}
                >
                  {t("settings.vpnMode.requestAdmin")}
                </button>
              )}
            </div>
          </label>
        </div>
      </div>

      <div className="settings-section">
        <div className="settings-label">{t("settings.hwid")}</div>
        <label className="toggle">
          <input
            type="checkbox"
            checked={state.settings.hwid_enabled}
            onChange={(e) => update("hwid_enabled", e.target.checked)}
          />
          <span className="toggle-slider" />
        </label>
        <div className="hwid-desc">{t("settings.hwidDesc")}</div>
        {state.settings.hwid_enabled && deviceInfo && (
          <div className="hwid-card">
            <div className="hwid-row">
              <span className="hwid-label">HWID</span>
              <span className="hwid-value">{deviceInfo.hwid}</span>
              <button
                className="hwid-copy-btn"
                onClick={() => {
                  navigator.clipboard.writeText(deviceInfo.hwid);
                  setHwidCopied(true);
                  setTimeout(() => setHwidCopied(false), 1500);
                }}
              >
                {hwidCopied ? t("settings.hwidCopied") : t("settings.hwidCopy")}
              </button>
            </div>
            <div className="hwid-row">
              <span className="hwid-label">{t("settings.hwidPlatform")}</span>
              <span>{deviceInfo.platform}</span>
            </div>
            <div className="hwid-row">
              <span className="hwid-label">{t("settings.hwidOsVersion")}</span>
              <span>{deviceInfo.os_version}</span>
            </div>
            <div className="hwid-row">
              <span className="hwid-label">{t("settings.hwidModel")}</span>
              <span>{deviceInfo.model}</span>
            </div>
          </div>
        )}
      </div>

      <div className="settings-section">
        <div className="settings-label">{t("settings.ports")}</div>
        <div className="port-inputs">
          <div className="form-group inline">
            <label className="form-label">SOCKS</label>
            <input
              className="form-input small"
              type="number"
              min={1}
              max={65535}
              value={state.settings.socks_port}
              onChange={(e) => update("socks_port", parseInt(e.target.value) || 0)}
            />
          </div>
          <div className="form-group inline">
            <label className="form-label">HTTP</label>
            <input
              className="form-input small"
              type="number"
              min={1}
              max={65535}
              value={state.settings.http_port}
              onChange={(e) => update("http_port", parseInt(e.target.value) || 0)}
            />
          </div>
        </div>
      </div>

      <div className="settings-section">
        <div className="settings-label">{t("settings.autoConnect")}</div>
        <label className="toggle">
          <input
            type="checkbox"
            checked={state.settings.auto_connect}
            onChange={(e) => update("auto_connect", e.target.checked)}
          />
          <span className="toggle-slider" />
        </label>
      </div>

      <div className="settings-section">
        <div className="settings-label">{t("settings.autoReconnect")}</div>
        <label className="toggle">
          <input
            type="checkbox"
            checked={state.settings.auto_reconnect}
            onChange={(e) => update("auto_reconnect", e.target.checked)}
          />
          <span className="toggle-slider" />
        </label>
      </div>

      <div className="settings-section">
        <div className="settings-label">{t("settings.language")}</div>
        <div className="core-radio-group">
          <label className={`core-radio ${state.settings.language === "en" ? "active" : ""}`}>
            <input
              type="radio"
              name="lang"
              checked={state.settings.language === "en"}
              onChange={() => update("language", "en")}
            />
            <span>English</span>
          </label>
          <label className={`core-radio ${state.settings.language === "ru" ? "active" : ""}`}>
            <input
              type="radio"
              name="lang"
              checked={state.settings.language === "ru"}
              onChange={() => update("language", "ru")}
            />
            <span>Русский</span>
          </label>
        </div>
      </div>

    </div>
  );
}
