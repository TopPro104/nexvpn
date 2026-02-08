import React, { createContext, useContext, useReducer, useCallback, useEffect } from "react";
import {
  api,
  ServerInfo,
  StatusResponse,
  SubscriptionInfo,
  Settings,
} from "../api/tauri";
import { setLang, Lang } from "../i18n/translations";

// ── State ──────────────────────────────────────

export type Page = "home" | "subscriptions" | "settings" | "logs" | "stats";

export interface AppState {
  page: Page;
  servers: ServerInfo[];
  subscriptions: SubscriptionInfo[];
  selectedServerId: string | null;
  connected: boolean;
  connectedAt: number | null; // timestamp (Date.now()) when connection started
  serverName: string | null;
  coreType: string;
  socksPort: number;
  httpPort: number;
  settings: Settings;
  toasts: Toast[];
  langTick: number; // bumped to force re-render on language change
  speedHistory: number[]; // last 60 download speed samples for sparkline
}

export interface Toast {
  id: number;
  message: string;
  type: "info" | "success" | "error";
}

const defaultSettings: Settings = {
  theme: "dark",
  style: "default",
  socks_port: 10808,
  http_port: 10809,
  auto_connect: false,
  language: "en",
  vpn_mode: "proxy",
  auto_reconnect: false,
  hwid_enabled: true,
  animation: "smooth",
};

const initialState: AppState = {
  page: "home",
  servers: [],
  subscriptions: [],
  selectedServerId: null,
  connected: false,
  connectedAt: null,
  serverName: null,
  coreType: "SingBox",
  socksPort: 10808,
  httpPort: 10809,
  settings: defaultSettings,
  toasts: [],
  langTick: 0,
  speedHistory: [],
};

// ── Actions ────────────────────────────────────

type Action =
  | { type: "SET_PAGE"; page: Page }
  | { type: "SET_SERVERS"; servers: ServerInfo[] }
  | { type: "ADD_SERVERS"; servers: ServerInfo[] }
  | { type: "REMOVE_SERVER"; id: string }
  | { type: "UPDATE_LATENCIES"; results: [string, number | null][] }
  | { type: "SET_SUBSCRIPTIONS"; subs: SubscriptionInfo[] }
  | { type: "SET_STATUS"; status: StatusResponse }
  | { type: "SELECT_SERVER"; id: string | null }
  | { type: "SET_SETTINGS"; settings: Settings }
  | { type: "ADD_TOAST"; toast: Omit<Toast, "id"> }
  | { type: "REMOVE_TOAST"; id: number }
  | { type: "BUMP_LANG" }
  | { type: "PUSH_SPEED"; speed: number };

let toastId = 0;

function reducer(state: AppState, action: Action): AppState {
  switch (action.type) {
    case "SET_PAGE":
      return { ...state, page: action.page };
    case "SET_SERVERS":
      return { ...state, servers: action.servers };
    case "ADD_SERVERS":
      return { ...state, servers: [...state.servers, ...action.servers] };
    case "REMOVE_SERVER":
      return {
        ...state,
        servers: state.servers.filter((s) => s.id !== action.id),
        selectedServerId:
          state.selectedServerId === action.id
            ? null
            : state.selectedServerId,
      };
    case "UPDATE_LATENCIES": {
      const map = new Map(action.results);
      return {
        ...state,
        servers: state.servers.map((s) =>
          map.has(s.id) ? { ...s, latency_ms: map.get(s.id)! } : s
        ),
      };
    }
    case "SET_SUBSCRIPTIONS":
      return { ...state, subscriptions: action.subs };
    case "SET_STATUS": {
      const wasConnected = state.connected;
      const nowConnected = action.status.connected;
      // Set connectedAt only on fresh connection; keep existing on status refresh
      let connectedAt = state.connectedAt;
      if (nowConnected && !wasConnected) {
        connectedAt = Date.now();
      } else if (!nowConnected) {
        connectedAt = null;
      }
      const speedHistory = nowConnected ? state.speedHistory : [];
      return {
        ...state,
        connected: nowConnected,
        connectedAt,
        serverName: action.status.server_name,
        coreType: action.status.core_type,
        socksPort: action.status.socks_port,
        httpPort: action.status.http_port,
        speedHistory,
      };
    }
    case "SELECT_SERVER":
      return { ...state, selectedServerId: action.id };
    case "SET_SETTINGS":
      return { ...state, settings: action.settings };
    case "ADD_TOAST":
      return {
        ...state,
        toasts: [...state.toasts, { ...action.toast, id: ++toastId }],
      };
    case "REMOVE_TOAST":
      return {
        ...state,
        toasts: state.toasts.filter((t) => t.id !== action.id),
      };
    case "BUMP_LANG":
      return { ...state, langTick: state.langTick + 1 };
    case "PUSH_SPEED": {
      const hist = [...state.speedHistory, action.speed];
      return { ...state, speedHistory: hist.slice(-60) };
    }
    default:
      return state;
  }
}

// ── Context ────────────────────────────────────

interface AppContextValue {
  state: AppState;
  dispatch: React.Dispatch<Action>;
  toast: (message: string, type?: "info" | "success" | "error") => void;
}

const AppCtx = createContext<AppContextValue>({
  state: initialState,
  dispatch: () => {},
  toast: () => {},
});

export function useApp() {
  return useContext(AppCtx);
}

export function AppProvider({ children }: { children: React.ReactNode }) {
  const [state, dispatch] = useReducer(reducer, initialState);

  const toast = useCallback(
    (message: string, type: "info" | "success" | "error" = "info") => {
      dispatch({ type: "ADD_TOAST", toast: { message, type } });
    },
    []
  );

  // Sync language whenever settings change
  useEffect(() => {
    setLang(state.settings.language as Lang);
  }, [state.settings.language]);

  // Load initial data
  useEffect(() => {
    (async () => {
      try {
        const [servers, status, settings, subs] = await Promise.all([
          api.getServers(),
          api.getStatus(),
          api.getSettings(),
          api.getSubscriptions(),
        ]);
        dispatch({ type: "SET_SERVERS", servers });
        dispatch({ type: "SET_STATUS", status });
        dispatch({ type: "SET_SETTINGS", settings });
        dispatch({ type: "SET_SUBSCRIPTIONS", subs });
        setLang(settings.language as Lang);
      } catch (e) {
        toast(`Init failed: ${e}`, "error");
      }
    })();
  }, [toast]);

  return (
    <AppCtx.Provider value={{ state, dispatch, toast }}>
      {children}
    </AppCtx.Provider>
  );
}
