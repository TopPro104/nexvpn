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
import { ToastStack } from "./components/ui/Toast";
import { useCallback, useRef, useEffect } from "react";
import { StatusResponse, api } from "./api/tauri";

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
