import { useState } from "react";
import { useApp } from "../../context/AppContext";
import { api } from "../../api/tauri";
import { t } from "../../i18n/translations";
import { ShieldIcon, CheckCircleIcon, AlertTriangleIcon, XCircleIcon } from "../ui/Icons";

interface CheckResult {
  label: string;
  description: string;
  status: "pass" | "warn" | "fail" | "pending";
  detail: string;
}

export function PrivacyPage() {
  const { state, toast } = useApp();
  void state.langTick;
  const [running, setRunning] = useState(false);
  const [score, setScore] = useState<number | null>(null);
  const [checks, setChecks] = useState<CheckResult[]>([]);

  const runChecks = async () => {
    if (!state.connected) {
      toast(t("privacy.connectFirst"), "error");
      return;
    }
    setRunning(true);
    setScore(null);
    setChecks([]);

    const results: CheckResult[] = [];
    let points = 0;

    // Check 1: IP Address
    try {
      const ip = await api.checkIp();
      results.push({
        label: t("privacy.ipCheck"),
        description: t("privacy.ipDesc"),
        status: "pass",
        detail: `${ip.ip} (${ip.city}, ${ip.country})`,
      });
      points += 25;
    } catch {
      results.push({
        label: t("privacy.ipCheck"),
        description: t("privacy.ipDesc"),
        status: "fail",
        detail: "Failed to check IP",
      });
    }
    setChecks([...results]);

    // Check 2: DNS Leak
    try {
      const dns = await api.checkDnsLeak();
      const leaked = dns.leaked;
      results.push({
        label: t("privacy.dnsCheck"),
        description: t("privacy.dnsDesc"),
        status: leaked ? "warn" : "pass",
        detail: leaked ? t("privacy.leakWarning") : t("privacy.noLeak"),
      });
      points += leaked ? 10 : 25;
    } catch {
      results.push({
        label: t("privacy.dnsCheck"),
        description: t("privacy.dnsDesc"),
        status: "warn",
        detail: "Could not verify DNS",
      });
      points += 10;
    }
    setChecks([...results]);

    // Check 3: Encryption
    const selectedServer = state.servers.find(s => s.id === state.selectedServerId);
    const protocol = selectedServer?.protocol || "";
    const isEncrypted = ["vless", "vmess", "trojan", "hysteria2", "tuic"].includes(protocol);
    results.push({
      label: t("privacy.encryption"),
      description: t("privacy.encDesc"),
      status: isEncrypted ? "pass" : "warn",
      detail: isEncrypted
        ? `${t("privacy.encrypted")} (${protocol.toUpperCase()})`
        : `${protocol.toUpperCase()} - verify encryption`,
    });
    points += isEncrypted ? 25 : 10;
    setChecks([...results]);

    // Check 4: VPN Mode
    const isTun = state.settings.vpn_mode === "tun";
    results.push({
      label: t("privacy.killSwitch"),
      description: t("privacy.killDesc"),
      status: isTun ? "pass" : "warn",
      detail: isTun ? t("privacy.tunActive") : t("privacy.proxyActive"),
    });
    points += isTun ? 25 : 15;
    setChecks([...results]);

    setScore(Math.min(100, points));
    setRunning(false);
  };

  const getScoreColor = (s: number) => {
    if (s >= 80) return "var(--success)";
    if (s >= 50) return "var(--warning)";
    return "var(--danger)";
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case "pass": return <CheckCircleIcon size={20} color="var(--success)" />;
      case "warn": return <AlertTriangleIcon size={20} color="var(--warning)" />;
      case "fail": return <XCircleIcon size={20} color="var(--danger)" />;
      default: return <ShieldIcon size={20} color="var(--text-muted)" />;
    }
  };

  // SVG gauge parameters
  const radius = 70;
  const circumference = 2 * Math.PI * radius;
  const progress = score !== null ? (score / 100) * circumference : 0;

  return (
    <div className="privacy-page">
      <div className="privacy-header">
        <h2><ShieldIcon size={24} /> {t("privacy.title")}</h2>
      </div>

      <div className="privacy-score-section">
        <div className="privacy-gauge">
          <svg width="180" height="180" viewBox="0 0 180 180">
            <circle
              cx="90" cy="90" r={radius}
              fill="none"
              stroke="var(--border)"
              strokeWidth="8"
            />
            {score !== null && (
              <circle
                cx="90" cy="90" r={radius}
                fill="none"
                stroke={getScoreColor(score)}
                strokeWidth="8"
                strokeLinecap="round"
                strokeDasharray={circumference}
                strokeDashoffset={circumference - progress}
                transform="rotate(-90 90 90)"
                style={{ transition: "stroke-dashoffset 1s ease, stroke 0.5s ease" }}
              />
            )}
            <text
              x="90" y="85"
              textAnchor="middle"
              fill={score !== null ? getScoreColor(score) : "var(--text-muted)"}
              fontSize="36"
              fontWeight="700"
            >
              {score !== null ? score : "—"}
            </text>
            <text
              x="90" y="108"
              textAnchor="middle"
              fill="var(--text-secondary)"
              fontSize="12"
            >
              {t("privacy.score")}
            </text>
          </svg>
        </div>

        <button
          className={`btn btn-md ${running ? "btn-secondary" : "btn-primary"}`}
          onClick={runChecks}
          disabled={running}
        >
          {running ? t("privacy.running") : t("privacy.runChecks")}
        </button>
      </div>

      {!state.connected && checks.length === 0 && (
        <div className="privacy-hint">
          {t("privacy.connectFirst")}
        </div>
      )}

      {checks.length > 0 && (
        <div className="privacy-checks">
          {checks.map((check, i) => (
            <div key={i} className={`privacy-check-card ${check.status}`}>
              <div className="privacy-check-icon">
                {getStatusIcon(check.status)}
              </div>
              <div className="privacy-check-info">
                <div className="privacy-check-label">{check.label}</div>
                <div className="privacy-check-desc">{check.description}</div>
                <div className="privacy-check-detail">{check.detail}</div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
