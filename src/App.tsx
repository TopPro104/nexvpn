import { AppProvider, useApp } from "./context/AppContext";
import { useTheme } from "./hooks/useTheme";
import { useStatus } from "./hooks/useStatus";
import { Layout } from "./components/layout/Layout";
import { StatusPanel } from "./components/home/StatusPanel";
import { TrafficPanel } from "./components/home/TrafficPanel";
import { ServerList } from "./components/home/ServerList";
import { QuickConnect } from "./components/home/QuickConnect";
import { SubList } from "./components/subscriptions/SubList";
import { SettingsPage } from "./components/settings/SettingsPage";
import { LogsPage } from "./components/logs/LogsPage";
import { StatsPage } from "./components/stats/StatsPage";
import { RoutingPage } from "./components/routing/RoutingPage";
import { OnboardingOverlay } from "./components/onboarding/OnboardingOverlay";
import { ToastStack } from "./components/ui/Toast";
import { useCallback, useRef, useEffect, useState } from "react";
import { StatusResponse, api } from "./api/tauri";
import { onOpenUrl, getCurrent as getDeepLinkUrls } from "@tauri-apps/plugin-deep-link";
import { listen } from "@tauri-apps/api/event";
import { t } from "./i18n/translations";
import { Page } from "./context/AppContext";

function AppContent() {
  const { state, dispatch, toast } = useApp();
  const reconnectingRef = useRef(false);
  const prevConnectedRef = useRef(state.connected);
  const [showOnboarding, setShowOnboarding] = useState(false);

  // Show onboarding when state loads and is not completed
  useEffect(() => {
    if (!state.onboardingCompleted) {
      setShowOnboarding(true);
    }
  }, [state.onboardingCompleted]);

  // Apply theme + style + animation
  useTheme(state.settings.theme, state.settings.style, state.settings.animation);

  // Poll status
  const handleStatus = useCallback(
    (status: StatusResponse) => dispatch({ type: "SET_STATUS", status }),
    [dispatch]
  );
  useStatus(handleStatus, 5000);

  // Auto-reconnect: if connection dropped unexpectedly and setting is enabled
  useEffect(() => {
    const wasConnected = prevConnectedRef.current;
    prevConnectedRef.current = state.connected;

    if (
      wasConnected &&
      !state.connected &&
      state.settings.auto_reconnect &&
      state.selectedServerId &&
      !reconnectingRef.current
    ) {
      reconnectingRef.current = true;
      toast("Reconnecting...", "info");
      api
        .connect(state.selectedServerId)
        .then((status) => dispatch({ type: "SET_STATUS", status }))
        .catch(() => {})
        .finally(() => {
          reconnectingRef.current = false;
        });
    }
  }, [state.connected, state.settings.auto_reconnect, state.selectedServerId, dispatch, toast]);

  // Deep link handler: nexvpn://import/SUBSCRIPTION_URL
  const processedUrls = useRef(new Set<string>());

  const handleDeepLink = useCallback(async (raw: string) => {
    // Deduplicate
    if (processedUrls.current.has(raw)) return;
    processedUrls.current.add(raw);

    const match = raw.match(/^nexvpn:\/\/import\/(.+)$/i);
    if (!match) return;
    const subUrl = decodeURIComponent(match[1]);
    toast(`${t("toast.subAdded")}...`, "info");
    try {
      await api.addSubscription(subUrl);
      const [servers, subs] = await Promise.all([
        api.getServers(),
        api.getSubscriptions(),
      ]);
      dispatch({ type: "SET_SERVERS", servers });
      dispatch({ type: "SET_SUBSCRIPTIONS", subs });
      dispatch({ type: "SET_PAGE", page: "subscriptions" });
      toast(t("toast.subAdded"), "success");
    } catch (e) {
      toast(`${e}`, "error");
    }
  }, [dispatch, toast]);

  useEffect(() => {
    // 1. Check URLs that launched the app
    getDeepLinkUrls()
      .then((urls) => urls?.forEach((u) => handleDeepLink(u)))
      .catch(() => {});

    // 2. Listen for deep-link plugin events (app already running)
    let unlistenOpen: (() => void) | undefined;
    onOpenUrl((urls) => urls.forEach((u) => handleDeepLink(u)))
      .then((fn) => { unlistenOpen = fn; });

    // 3. Listen for single-instance forwarded URLs
    let unlistenSingle: (() => void) | undefined;
    listen<string>("deep-link-received", (e) => handleDeepLink(e.payload))
      .then((fn) => { unlistenSingle = fn; });

    return () => {
      unlistenOpen?.();
      unlistenSingle?.();
    };
  }, [handleDeepLink]);

  // ── Android Quick Settings Tile action handler ──────────
  // Use refs so the polling closure always sees latest state without re-creating
  const connectedRef = useRef(state.connected);
  const selectedServerRef = useRef(state.selectedServerId);
  const serversRef = useRef(state.servers);
  const tileProcessingRef = useRef(false);

  useEffect(() => { connectedRef.current = state.connected; }, [state.connected]);
  useEffect(() => { selectedServerRef.current = state.selectedServerId; }, [state.selectedServerId]);
  useEffect(() => { serversRef.current = state.servers; }, [state.servers]);

  useEffect(() => {
    let cancelled = false;

    const checkTileAction = async () => {
      if (tileProcessingRef.current) return; // prevent concurrent processing
      try {
        const action = await api.readTileAction();
        if (!action || cancelled) return;

        tileProcessingRef.current = true;
        console.log("Tile action:", action, "connected:", connectedRef.current);

        if (action === "connect") {
          // Always get fresh status from backend before acting
          const currentStatus = await api.getStatus();
          dispatch({ type: "SET_STATUS", status: currentStatus });

          if (!currentStatus.connected) {
            let serverId = selectedServerRef.current;
            if (!serverId && serversRef.current.length > 0) {
              serverId = serversRef.current[0].id;
              dispatch({ type: "SELECT_SERVER", id: serverId });
            }
            if (serverId) {
              const status = await api.connect(serverId);
              dispatch({ type: "SET_STATUS", status });
            }
          }
        } else if (action === "disconnect") {
          // Always get fresh status before disconnecting
          const currentStatus = await api.getStatus();
          if (currentStatus.connected) {
            const status = await api.disconnect();
            dispatch({ type: "SET_STATUS", status });
          } else {
            // Sync frontend state with reality
            dispatch({ type: "SET_STATUS", status: currentStatus });
          }
        }
      } catch (e) {
        console.error("Tile action error:", e);
      } finally {
        tileProcessingRef.current = false;
      }
    };

    // Check immediately on mount (app just launched from tile)
    checkTileAction();

    // Poll every 1.5s — fast enough to catch tile actions, light enough to not drain battery
    const interval = setInterval(checkTileAction, 1500);

    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [dispatch]); // Only dispatch — refs handle the rest

  // ── Keyboard shortcuts ──────────────────────
  // No modifier for navigation (1-6), Shift+D connect, Shift+F search
  // Uses capture phase to intercept before WebView consumes the event
  useEffect(() => {
    const pages: Page[] = ["home", "subscriptions", "routing", "stats", "logs", "settings"];

    const handler = (e: KeyboardEvent) => {
      // Escape always blurs focused inputs
      if (e.key === "Escape") {
        (e.target as HTMLElement)?.blur();
        return;
      }

      // Ignore shortcuts while typing in inputs
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

      // No modifier keys pressed — bare number keys 1-6 navigate pages
      if (!e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
        const num = parseInt(e.key);
        if (num >= 1 && num <= 6) {
          e.preventDefault();
          dispatch({ type: "SET_PAGE", page: pages[num - 1] });
          return;
        }
      }

      // Shift+key shortcuts (no Ctrl/Alt — avoids WebView interception)
      if (e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey) {
        // Shift+D — toggle connect/disconnect
        if (e.key === "D") {
          e.preventDefault();
          if (state.connected) {
            api.disconnect().then(s => dispatch({ type: "SET_STATUS", status: s }));
          } else if (state.selectedServerId) {
            api.connect(state.selectedServerId).then(s => dispatch({ type: "SET_STATUS", status: s }));
          }
          return;
        }
        // Shift+F — focus search on home page
        if (e.key === "F") {
          e.preventDefault();
          dispatch({ type: "SET_PAGE", page: "home" });
          setTimeout(() => {
            const input = document.querySelector<HTMLInputElement>(".search-input");
            input?.focus();
          }, 50);
          return;
        }
      }
    };

    document.addEventListener("keydown", handler, true);
    return () => document.removeEventListener("keydown", handler, true);
  }, [state.connected, state.selectedServerId, dispatch]);

  const renderPage = () => {
    switch (state.page) {
      case "home":
        return (
          <div className={`home-page ${state.connected ? "vpn-active" : ""}`}>
            <div className="home-left">
              <StatusPanel />
              <TrafficPanel />
              <QuickConnect />
            </div>
            <div className="home-right">
              <ServerList />
            </div>
          </div>
        );
      case "subscriptions":
        return <SubList />;
      case "settings":
        return <SettingsPage />;
      case "routing":
        return <RoutingPage />;
      case "logs":
        return <LogsPage />;
      case "stats":
        return <StatsPage />;
    }
  };

  return (
    <Layout>
      {renderPage()}
      <ToastStack />
      {showOnboarding && (
        <OnboardingOverlay onComplete={() => setShowOnboarding(false)} />
      )}
    </Layout>
  );
}

export default function App() {
  return (
    <AppProvider>
      <AppContent />
    </AppProvider>
  );
}
