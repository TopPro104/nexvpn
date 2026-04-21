import { invoke } from "@tauri-apps/api/core";

// ── Types ──────────────────────────────────────

export interface ServerInfo {
  id: string;
  name: string;
  address: string;
  port: number;
  protocol: string;
  transport: string;
  security: string; // "tls" | "reality" | "none"
  latency_ms: number | null;
  subscription_id: string | null;
  favorite: boolean;
}

export interface StatusResponse {
  connected: boolean;
  server_name: string | null;
  core_type: string;
  socks_port: number;
  http_port: number;
}

export interface SubscriptionInfo {
  id: string;
  name: string;
  url: string;
  server_count: number;
  updated_at: number | null;
  update_interval: number | null;
  upload: number | null;
  download: number | null;
  total: number | null;
  expire: number | null;
  web_page_url: string | null;
  support_url: string | null;
  announce: string | null;
  refill_date: number | null;
}

export interface TrafficStats {
  upload: number;
  download: number;
}

export interface Settings {
  theme: string;
  style: string;
  socks_port: number;
  http_port: number;
  port_mode: string; // "auto" | "manual"
  auto_connect: boolean;
  language: string;
  vpn_mode: string; // "proxy" | "tun"
  auto_reconnect: boolean;
  hwid_enabled: boolean;
  animation: string; // "none" | "smooth" | "energetic"
  stealth_mode: boolean;
  per_app_mode: string; // "all" | "include" | "exclude"
  per_app_list: string[];
  happ_ua: boolean;
}

export interface InstalledApp {
  package_name: string;
  label: string;
  icon: string; // base64 PNG
}

export interface DeviceInfo {
  hwid: string;
  platform: string;
  os_version: string;
  model: string;
  user_agent: string;
}

export interface ConnectionRecord {
  server_name: string;
  server_address: string;
  protocol: string;
  core_type: string;
  vpn_mode: string;
  connected_at: number;
  disconnected_at: number | null;
  upload_bytes: number;
  download_bytes: number;
}

export type RuleAction = "proxy" | "direct" | "block";

export interface RoutingRule {
  id: string;
  domain: string;
  action: RuleAction;
  enabled: boolean;
}

export interface RoutingRulesResponse {
  rules: RoutingRule[];
  default_route: string;
}

export interface IpCheckResult {
  ip: string;
  country: string;
  city: string;
  lat: number;
  lon: number;
}

export interface DnsLeakResult {
  leaked: boolean;
  dns_servers: string[];
}

export interface SpeedTestResult {
  download_mbps: number;
  upload_mbps: number;
  ping_ms: number;
}

export interface UpdateCheckResult {
  has_update: boolean;
  latest_version: string;
  current_version: string;
  changelog: string;
  download_url: string;
}

export interface DailyTraffic {
  date: string;
  upload: number;
  download: number;
}

export interface ServerUsageStat {
  server_name: string;
  protocol: string;
  connection_count: number;
  total_traffic: number;
}

// ── API calls ──────────────────────────────────

export const api = {
  getServers: () => invoke<ServerInfo[]>("get_servers"),

  addLinks: (links: string) => invoke<ServerInfo[]>("add_links", { links }),

  addSubscription: (url: string, name?: string) =>
    invoke<ServerInfo[]>("add_subscription", { url, name: name || null }),

  removeServer: (serverId: string) =>
    invoke<void>("remove_server", { serverId }),

  connect: (serverId: string) =>
    invoke<StatusResponse>("connect", { serverId }),

  disconnect: () => invoke<StatusResponse>("disconnect"),

  getStatus: () => invoke<StatusResponse>("get_status"),

  setCoreType: (core: string) =>
    invoke<string>("set_core_type", { core }),

  pingServer: (serverId: string) =>
    invoke<number | null>("ping_server", { serverId }),

  pingAllServers: () =>
    invoke<[string, number | null][]>("ping_all_servers"),

  getSubscriptions: () =>
    invoke<SubscriptionInfo[]>("get_subscriptions"),

  updateSubscription: (subscriptionId: string) =>
    invoke<ServerInfo[]>("update_subscription", { subscriptionId }),

  deleteSubscription: (subscriptionId: string) =>
    invoke<void>("delete_subscription", { subscriptionId }),

  autoSelectServer: () => invoke<ServerInfo>("auto_select_server"),

  exportConfig: () => invoke<string>("export_config"),

  importConfig: (data: string) =>
    invoke<string>("import_config", { data }),

  getTrafficStats: () => invoke<TrafficStats>("get_traffic_stats"),

  getSettings: () => invoke<Settings>("get_settings"),

  saveSettings: (settings: Settings) =>
    invoke<void>("save_settings", { settings }),

  getLogs: () => invoke<string[]>("get_logs"),

  clearLogs: () => invoke<void>("clear_logs"),

  getAppLogs: () => invoke<string[]>("get_app_logs"),

  clearAppLogs: () => invoke<void>("clear_app_logs"),

  getConnectionHistory: () =>
    invoke<ConnectionRecord[]>("get_connection_history"),

  clearConnectionHistory: () =>
    invoke<void>("clear_connection_history"),

  getDeviceInfo: () => invoke<DeviceInfo>("get_device_info"),

  openUrl: (url: string) => invoke<void>("open_url", { url }),

  getRoutingRules: () => invoke<RoutingRulesResponse>("get_routing_rules"),

  saveRoutingRules: (rules: RoutingRule[], defaultRoute: string) =>
    invoke<void>("save_routing_rules", { rules, defaultRoute }),

  getOnboardingCompleted: () => invoke<boolean>("get_onboarding_completed"),

  completeOnboarding: () => invoke<void>("complete_onboarding"),

  isAdmin: () => invoke<boolean>("is_admin"),

  restartAsAdmin: () => invoke<void>("restart_as_admin"),

  // Privacy Shield
  checkIp: () => invoke<IpCheckResult>("check_ip"),
  checkDnsLeak: () => invoke<DnsLeakResult>("check_dns_leak"),

  // Speed Test
  runSpeedTest: () => invoke<SpeedTestResult>("run_speed_test"),

  // Auto-Update
  checkForUpdates: () => invoke<UpdateCheckResult>("check_for_updates"),

  // Favorites
  toggleFavorite: (serverId: string) =>
    invoke<boolean>("toggle_favorite", { serverId }),

  // Dashboard stats
  getDailyTraffic: () => invoke<DailyTraffic[]>("get_daily_traffic"),
  getServerUsageStats: () => invoke<ServerUsageStat[]>("get_server_usage_stats"),

  // Android tile action
  readTileAction: () => invoke<string | null>("read_tile_action"),

  // Persisted active server
  getActiveServerId: () => invoke<string | null>("get_active_server_id"),

  setSelectedServer: (serverId: string | null) =>
    invoke<void>("set_selected_server", { serverId }),

  // Per-app VPN
  getInstalledApps: () => invoke<InstalledApp[]>("get_installed_apps"),

  // VPN ping
  pingThroughVpn: () => invoke<number>("ping_through_vpn"),

  // Share
  getServerLink: (serverId: string) => invoke<string>("get_server_link", { serverId }),

  // Xposed module status
  getXposedStatus: () => invoke<{ active: boolean; hooked_apps: string[] }>("get_xposed_status"),
};
