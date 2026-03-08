import { useState, useEffect } from "react";
import { api, UpdateCheckResult } from "../../api/tauri";
import { t } from "../../i18n/translations";
import { DownloadCloudIcon, CheckCircleIcon } from "../ui/Icons";

function compareVersions(a: string, b: string): number {
  const pa = a.replace(/^v/, "").split(".").map(Number);
  const pb = b.replace(/^v/, "").split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const na = pa[i] || 0;
    const nb = pb[i] || 0;
    if (na > nb) return 1;
    if (na < nb) return -1;
  }
  return 0;
}

export function UpdateChecker() {
  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<UpdateCheckResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    handleCheck();
  }, []);

  const handleCheck = async () => {
    setChecking(true);
    setError(null);
    try {
      const res = await api.checkForUpdates();
      setResult(res);
    } catch (e) {
      setError(`${e}`);
    }
    setChecking(false);
  };

  const handleDownload = () => {
    if (result?.download_url) {
      api.openUrl(result.download_url).catch(() => {});
    }
  };

  const isDev = result && !result.has_update &&
    result.latest_version &&
    result.latest_version !== "0.0.0" &&
    compareVersions(result.current_version, result.latest_version) > 0;

  const isUpToDate = result && !result.has_update && !isDev;

  return (
    <div className="update-checker">
      <div className="update-checker-header">
        <DownloadCloudIcon size={18} />
        <span className="update-checker-title">{t("update.title")}</span>
        {isDev && (
          <span className="dev-badge">
            <span className="dev-badge-dot" />
            DEV
          </span>
        )}
      </div>

      {isDev && (
        <div className="update-status dev-version">
          <span className="dev-version-label">v{result.current_version}</span>
          <span className="dev-version-hint">
            {t("update.devHint")} (release: v{result.latest_version})
          </span>
        </div>
      )}

      {isUpToDate && (
        <div className="update-status up-to-date">
          <CheckCircleIcon size={16} color="var(--success)" />
          <span>{t("update.upToDate")}</span>
          <span className="update-version">v{result.current_version}</span>
        </div>
      )}

      {result && result.has_update && (
        <div className="update-status has-update">
          <div className="update-versions">
            <span className="update-current">v{result.current_version}</span>
            <span className="update-arrow">&rarr;</span>
            <span className="update-latest">v{result.latest_version}</span>
          </div>
          {result.changelog && (
            <div className="update-changelog">{result.changelog}</div>
          )}
          <button className="btn btn-sm btn-primary" onClick={handleDownload}>
            {t("update.download")}
          </button>
        </div>
      )}

      {error && (
        <div className="update-status update-error">
          <span>{t("update.error")}</span>
        </div>
      )}

      <button
        className="btn btn-sm btn-secondary update-check-btn"
        onClick={handleCheck}
        disabled={checking}
      >
        {checking ? t("update.checking") : t("update.check")}
      </button>
    </div>
  );
}
