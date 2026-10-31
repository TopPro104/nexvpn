import { useEffect, useState } from "react";
import { useApp } from "../../context/AppContext";
import { api } from "../../api/tauri";
import { t } from "../../i18n/translations";
import { AlertTriangleIcon } from "../ui/Icons";

/** Warns when zapret / GoodbyeDPI is running: they rewrite packets and break the VPN. */
export function DpiBypassBanner() {
  const { state } = useApp();
  void state.langTick;
  const [tools, setTools] = useState<string[]>([]);
  const [dismissed, setDismissed] = useState("");

  // Re-check on connect/disconnect and every minute (the user may start zapret any time)
  useEffect(() => {
    let alive = true;
    const check = () =>
      api.detectDpiBypass().then((found) => alive && setTools(found)).catch(() => {});
    check();
    const timer = setInterval(check, 60_000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [state.connected]);

  const key = tools.join(",");
  if (!key || key === dismissed) return null;

  return (
    <div className="announce-banner dpi-banner">
      <AlertTriangleIcon size={16} className="announce-banner-icon" />
      <div className="announce-banner-content">
        <strong>{t("dpi.warnTitle")}</strong>
        <div>{t("dpi.warnText").replace("{tools}", tools.join(", "))}</div>
      </div>
      <button className="btn btn-sm dpi-banner-dismiss" onClick={() => setDismissed(key)}>
        {t("dpi.dismiss")}
      </button>
    </div>
  );
}
