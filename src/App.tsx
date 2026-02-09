import { AppProvider, useApp } from "./context/AppContext";
import { useTheme } from "./hooks/useTheme";
import { useStatus } from "./hooks/useStatus";
import { Layout } from "./components/layout/Layout";
import { StatusPanel } from "./components/home/StatusPanel";
import { TrafficPanel } from "./components/home/TrafficPanel";
import { ServerList } from "./components/home/ServerList";
import { SubList } from "./components/subscriptions/SubList";
import { SettingsPage } from "./components/settings/SettingsPage";
import { LogsPage } from "./components/logs/LogsPage";
import { StatsPage } from "./components/stats/StatsPage";
import { RoutingPage } from "./components/routing/RoutingPage";
import { ToastStack } from "./components/ui/Toast";
import { useCallback, useRef, useEffect } from "react";
import { StatusResponse, api } from "./api/tauri";
import { onOpenUrl, getCurrent as getDeepLinkUrls } from "@tauri-apps/plugin-deep-link";
import { listen } from "@tauri-apps/api/event";
import { t } from "./i18n/translations";

function AppContent() {
  const { state, dispatch, toast } = useApp();
  const reconnectingRef = useRef(false);
  const prevConnectedRef = useRef(state.connected);

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

  const renderPage = () => {
    switch (state.page) {
      case "home":
        return (
          <div className="home-page">
            <StatusPanel />
            <TrafficPanel />
            <ServerList />
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
