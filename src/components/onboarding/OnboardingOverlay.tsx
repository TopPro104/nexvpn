import { useState, useEffect, useCallback, useRef } from "react";
import { useApp, Page } from "../../context/AppContext";
import { api, RuleAction } from "../../api/tauri";
import { t, setLang, Lang } from "../../i18n/translations";

interface Preset {
  id: string;
  name_en: string;
  name_ru: string;
  action: RuleAction;
  domains: string[];
}

const FALLBACK_ADS_PRESET: Preset = {
  id: "block_ads",
  name_en: "Block Ads",
  name_ru: "Блокировка рекламы",
  action: "block",
  domains: [
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "adnxs.com",
    "ads.yahoo.com",
    "moatads.com",
    "adcolony.com",
    "direct.yandex.ru",
  ],
};

const PRESETS_URL =
  "https://raw.githubusercontent.com/TopPro104/nexvpn/main/presets.json";

type Step = "lang" | "welcome" | "home" | "subscriptions" | "routing" | "settings" | "finish";

const TOUR_STEPS: Step[] = ["lang", "welcome", "home", "subscriptions", "routing", "settings", "finish"];

const STEP_PAGE_MAP: Partial<Record<Step, Page>> = {
  home: "home",
  subscriptions: "subscriptions",
  routing: "routing",
  settings: "settings",
};

const STEP_SELECTORS: Partial<Record<Step, string>> = {
  home: ".status-panel",
  subscriptions: ".sub-page, .sub-header",
  routing: ".routing-page",
  settings: ".settings-page",
};

interface Props {
  onComplete: () => void;
}

export function OnboardingOverlay({ onComplete }: Props) {
  const { state, dispatch, toast } = useApp();
  void state.langTick;

  const [stepIndex, setStepIndex] = useState(0);
  const [spotlightRect, setSpotlightRect] = useState<DOMRect | null>(null);
  const [adsPreset, setAdsPreset] = useState<Preset>(FALLBACK_ADS_PRESET);
  const [leaving, setLeaving] = useState(false);
  const [pendingStep, setPendingStep] = useState<number | null>(null);
  const overlayRef = useRef<HTMLDivElement>(null);

  const step = TOUR_STEPS[stepIndex];
  const isCard = step === "lang" || step === "welcome" || step === "finish";
  const isSpotlight = !isCard;

  // Fetch remote presets for "Block Ads" CTA
  useEffect(() => {
    fetch(PRESETS_URL)
      .then((res) => {
        if (!res.ok) throw new Error("HTTP " + res.status);
        return res.json();
      })
      .then((data: { presets: Preset[] }) => {
        const ads = data.presets?.find((p) => p.id === "block_ads");
        if (ads) setAdsPreset(ads);
      })
      .catch(() => {});
  }, []);

  // Navigate to the page for the current step + scroll to top
  useEffect(() => {
    const page = STEP_PAGE_MAP[step];
    if (page && state.page !== page) {
      dispatch({ type: "SET_PAGE", page });
    }
    const mc = document.querySelector(".main-content");
    if (mc) mc.scrollTop = 0;
  }, [step, state.page, dispatch]);

  // Measure spotlight target after page navigates
  useEffect(() => {
    const selector = STEP_SELECTORS[step];
    if (!selector) {
      setSpotlightRect(null);
      return;
    }

    setSpotlightRect(null);

    const measure = () => {
      const el = document.querySelector(selector);
      if (el) {
        setSpotlightRect(el.getBoundingClientRect());
      } else {
        setSpotlightRect(null);
      }
    };

    const timer = setTimeout(measure, 300);
    window.addEventListener("resize", measure);
    return () => {
      clearTimeout(timer);
      window.removeEventListener("resize", measure);
    };
  }, [step, stepIndex]);

  // Process pending step after leave animation
  useEffect(() => {
    if (!leaving || pendingStep === null) return;
    const timer = setTimeout(() => {
      setStepIndex(pendingStep);
      setPendingStep(null);
      setLeaving(false);
    }, 200);
    return () => clearTimeout(timer);
  }, [leaving, pendingStep]);

  const finish = useCallback(async () => {
    try {
      await api.completeOnboarding();
    } catch {
      // ignore
    }
    dispatch({ type: "SET_ONBOARDING_COMPLETED", completed: true });
    onComplete();
  }, [dispatch, onComplete]);

  const skip = useCallback(() => {
    finish();
  }, [finish]);

  const goToStep = useCallback((idx: number) => {
    setLeaving(true);
    setPendingStep(idx);
  }, []);

  const next = useCallback(() => {
    if (stepIndex < TOUR_STEPS.length - 1) {
      goToStep(stepIndex + 1);
    }
  }, [stepIndex, goToStep]);

  const selectLanguage = useCallback(async (lang: Lang) => {
    setLang(lang);
    dispatch({ type: "BUMP_LANG" });
    const newSettings = { ...state.settings, language: lang };
    dispatch({ type: "SET_SETTINGS", settings: newSettings });
    try {
      await api.saveSettings(newSettings);
    } catch {
      // ignore
    }
    next();
  }, [state.settings, dispatch, next]);

  const applyAdsPreset = useCallback(async () => {
    const existing = new Set(state.routingRules.map((r) => r.domain));
    const newRules = adsPreset.domains
      .filter((d) => !existing.has(d))
      .map((domain) => ({
        id: String(Date.now() + Math.random()),
        domain,
        action: adsPreset.action,
        enabled: true,
      }));

    const allRules = [...state.routingRules, ...newRules];
    try {
      await api.saveRoutingRules(allRules, state.defaultRoute);
      dispatch({ type: "SET_ROUTING_RULES", rules: allRules, defaultRoute: state.defaultRoute });
      toast(t("routing.saved"), "success");
    } catch (e) {
      toast(`${e}`, "error");
    }

    dispatch({ type: "SET_PAGE", page: "routing" });
    finish();
  }, [state.routingRules, state.defaultRoute, adsPreset, dispatch, toast, finish]);

  // Build clip-path for spotlight hole
  const backdropStyle: React.CSSProperties = {};
  if (spotlightRect) {
    const pad = 8;
    const x = spotlightRect.x - pad;
    const y = spotlightRect.y - pad;
    const w = spotlightRect.width + pad * 2;
    const h = spotlightRect.height + pad * 2;
    const r = 12;
    backdropStyle.clipPath = `polygon(
      0% 0%, 0% 100%, 100% 100%, 100% 0%, 0% 0%,
      ${x}px ${y + r}px,
      ${x + r}px ${y}px,
      ${x + w - r}px ${y}px,
      ${x + w}px ${y + r}px,
      ${x + w}px ${y + h - r}px,
      ${x + w - r}px ${y + h}px,
      ${x + r}px ${y + h}px,
      ${x}px ${y + h - r}px,
      ${x}px ${y + r}px
    )`;
  }

  // Tooltip positioning: clamp to viewport
  const tooltipStyle: React.CSSProperties = {};
  if (spotlightRect) {
    const vTop = Math.max(0, spotlightRect.top);
    const vBottom = Math.min(window.innerHeight, spotlightRect.bottom);
    const spaceBelow = window.innerHeight - vBottom;
    const spaceAbove = vTop;
    const tooltipH = 140;

    if (spaceBelow > tooltipH) {
      tooltipStyle.top = vBottom + 16;
    } else if (spaceAbove > tooltipH) {
      tooltipStyle.bottom = window.innerHeight - vTop + 16;
    } else {
      tooltipStyle.bottom = 16;
    }

    if (tooltipStyle.top !== undefined && (tooltipStyle.top as number) < 8) {
      tooltipStyle.top = 8;
    }

    tooltipStyle.left = Math.max(16, Math.min(spotlightRect.left, window.innerWidth - 336));
  }

  // Step dots (for spotlight steps only)
  const spotlightSteps = TOUR_STEPS.filter(
    (s): s is "home" | "subscriptions" | "routing" | "settings" =>
      s !== "lang" && s !== "welcome" && s !== "finish"
  );
  const spotlightStepIndex = spotlightSteps.indexOf(step as typeof spotlightSteps[number]);

  const stepText = (): string => {
    switch (step) {
      case "home": return t("onboarding.stepHome");
      case "subscriptions": return t("onboarding.stepSubs");
      case "routing": return t("onboarding.stepRouting");
      case "settings": return t("onboarding.stepSettings");
      default: return "";
    }
  };

  const contentClass = `onboarding-step-content ${leaving ? "leaving" : "entering"}`;

  // Card content (lang / welcome / finish)
  const renderCard = () => {
    if (step === "lang") {
      return (
        <div className="onboarding-card-wrapper">
          <div className="onboarding-card">
            <div className="onboarding-card-icon">N</div>
            <div className="onboarding-card-subtitle">NexVPN</div>
            <div className="onboarding-lang-title">Choose language / Выберите язык</div>
            <div className="onboarding-lang-buttons">
              <button className="onboarding-lang-btn" onClick={() => selectLanguage("en")}>
                <span className="onboarding-lang-flag">EN</span>
                <span className="onboarding-lang-label">English</span>
              </button>
              <button className="onboarding-lang-btn" onClick={() => selectLanguage("ru")}>
                <span className="onboarding-lang-flag">RU</span>
                <span className="onboarding-lang-label">Русский</span>
              </button>
            </div>
          </div>
        </div>
      );
    }

    if (step === "welcome") {
      return (
        <div className="onboarding-card-wrapper">
          <div className="onboarding-card">
            <div className="onboarding-card-title">{t("onboarding.welcome")}</div>
            <div className="onboarding-card-actions">
              <button className="btn btn-primary btn-md" onClick={next}>
                {t("onboarding.showMe")}
              </button>
              <button className="btn btn-ghost btn-md" onClick={skip}>
                {t("onboarding.skip")}
              </button>
            </div>
          </div>
        </div>
      );
    }

    // finish
    return (
      <div className="onboarding-card-wrapper">
        <div className="onboarding-card">
          <div className="onboarding-card-title">{t("onboarding.finish")}</div>
          <div className="onboarding-card-actions">
            <button className="btn btn-primary btn-md" onClick={applyAdsPreset}>
              {t("onboarding.blockAdsNow")}
            </button>
            <button className="btn btn-secondary btn-md" onClick={() => { dispatch({ type: "SET_PAGE", page: "home" }); finish(); }}>
              {t("onboarding.startBrowsing")}
            </button>
          </div>
        </div>
      </div>
    );
  };

  // Tooltip content (spotlight steps)
  const renderTooltip = () => (
    <div
      className="onboarding-tooltip"
      style={{ ...tooltipStyle, visibility: spotlightRect ? "visible" : "hidden" }}
    >
      <div className="onboarding-tooltip-text">{stepText()}</div>
      <div className="onboarding-tooltip-actions">
        <div className="onboarding-dots">
          {spotlightSteps.map((_, i) => (
            <div key={i} className={`onboarding-dot ${i === spotlightStepIndex ? "active" : ""}`} />
          ))}
        </div>
        <div className="onboarding-tooltip-btns">
          <button className="btn btn-ghost btn-sm" onClick={skip}>
            {t("onboarding.skip")}
          </button>
          <button className="btn btn-primary btn-sm" onClick={next}>
            {t("onboarding.next")}
          </button>
        </div>
      </div>
    </div>
  );

  return (
    <div className="onboarding-overlay" ref={overlayRef}>
      <div className="onboarding-backdrop" style={backdropStyle} />
      <div key={stepIndex} className={contentClass}>
        {isCard ? renderCard() : renderTooltip()}
      </div>
    </div>
  );
}
