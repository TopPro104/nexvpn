use serde::Serialize;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

use crate::core::manager::CoreManager;
use crate::proxy::{link_parser, models::*, subscription};
use crate::system::{hwid, proxy_setter};
use crate::testing::ping;

/// Shared app state managed by Tauri
pub struct AppContext {
    pub core: CoreManager,
    pub state: Arc<Mutex<AppState>>,
    /// App-level log buffer (subscription parsing, errors, etc.)
    pub app_logs: Arc<Mutex<Vec<String>>>,
}

/// Push a log line to the app log buffer
pub fn app_log(ctx: &AppContext, msg: String) {
    let logs = ctx.app_logs.clone();
    log::info!("[APP] {}", msg);
    tauri::async_runtime::spawn(async move {
        let mut buf = logs.lock().await;
        buf.push(msg);
        let blen = buf.len();
        if blen > 500 {
            buf.drain(0..blen - 400);
        }
    });
}

// ── Response types ─────────────────────────────────────

#[derive(Serialize)]
pub struct ServerInfo {
    pub id: String,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub protocol: String,
    pub latency_ms: Option<u32>,
    pub subscription_id: Option<String>,
    pub favorite: bool,
}

impl From<&Server> for ServerInfo {
    fn from(s: &Server) -> Self {
        ServerInfo {
            id: s.id.clone(),
            name: s.name.clone(),
            address: s.address.clone(),
            port: s.port,
            protocol: format!("{:?}", s.protocol).to_lowercase(),
            latency_ms: s.latency_ms,
            subscription_id: s.subscription_id.clone(),
            favorite: s.favorite,
        }
    }
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub connected: bool,
    pub server_name: Option<String>,
    pub core_type: String,
    pub socks_port: u16,
    pub http_port: u16,
}

#[derive(Serialize)]
pub struct SubscriptionInfo {
    pub id: String,
    pub name: String,
    pub url: String,
    pub server_count: usize,
    pub updated_at: Option<u64>,
    pub update_interval: Option<u64>,
    pub upload: Option<u64>,
    pub download: Option<u64>,
    pub total: Option<u64>,
    pub expire: Option<u64>,
    pub web_page_url: Option<String>,
    pub support_url: Option<String>,
    pub announce: Option<String>,
    pub refill_date: Option<u64>,
}

impl SubscriptionInfo {
    fn from_sub(sub: &Subscription, servers: &[Server]) -> Self {
        let server_count = servers.iter().filter(|s| s.subscription_id.as_deref() == Some(&sub.id)).count();
        SubscriptionInfo {
            id: sub.id.clone(),
            name: sub.name.clone(),
            url: sub.url.clone(),
            server_count,
            updated_at: sub.updated_at,
            update_interval: sub.update_interval,
            upload: sub.upload,
            download: sub.download,
            total: sub.total,
            expire: sub.expire,
            web_page_url: sub.web_page_url.clone(),
            support_url: sub.support_url.clone(),
            announce: sub.announce.clone(),
            refill_date: sub.refill_date,
        }
    }
}

#[derive(Serialize)]
pub struct IpCheckResult {
    pub ip: String,
    pub country: String,
    pub city: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Serialize)]
pub struct DnsLeakResult {
    pub leaked: bool,
    pub dns_servers: Vec<String>,
}

#[derive(Serialize)]
pub struct SpeedTestResult {
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub ping_ms: u32,
}

#[derive(Serialize)]
pub struct UpdateCheckResult {
    pub has_update: bool,
    pub latest_version: String,
    pub current_version: String,
    pub changelog: String,
    pub download_url: String,
}

#[derive(Serialize)]
pub struct DailyTraffic {
    pub date: String,
    pub upload: u64,
    pub download: u64,
}

#[derive(Serialize)]
pub struct ServerUsageStat {
    pub server_name: String,
    pub protocol: String,
    pub connection_count: u32,
    pub total_traffic: u64,
}

// ── Human-readable error messages ─────────────────────

fn humanize_core_error(raw: &str, tun_mode: bool) -> String {
    let lower = raw.to_lowercase();

    // TUN without admin rights
    if tun_mode && (lower.contains("access denied") || lower.contains("permission denied")
        || lower.contains("requires elevated") || lower.contains("operation not permitted")
        || lower.contains("wintun") || lower.contains("administrator"))
    {
        return "TUN mode requires administrator rights. Click \"Run as Administrator\" in VPN settings, or switch to Proxy mode.".to_string();
    }

    // wintun.dll missing
    if lower.contains("wintun.dll") || (tun_mode && lower.contains("not found") && lower.contains("dll")) {
        return "wintun.dll not found. TUN mode needs this file next to the app. Download it from wintun.net or switch to Proxy mode.".to_string();
    }

    // TUN + Xray
    if lower.contains("tun mode requires sing-box") {
        return "TUN mode only works with sing-box core. Switch to sing-box in Settings, or use Proxy mode with Xray.".to_string();
    }

    // Port already in use
    if lower.contains("address already in use") || lower.contains("bind") && lower.contains("failed") {
        return "Port is already in use by another app. Try switching to automatic ports in Settings, or change ports manually.".to_string();
    }

    // Binary not found
    if lower.contains("no such file") || lower.contains("cannot find") || lower.contains("not found") && (lower.contains("sing-box") || lower.contains("xray")) {
        return "VPN core binary not found. Reinstall the app or check that core files are in the binaries folder.".to_string();
    }

    // Connection refused / timeout (server issue)
    if lower.contains("connection refused") || lower.contains("connection timed out") || lower.contains("timed out") {
        return "Cannot reach the server. It may be down, blocked, or your internet is offline.".to_string();
    }

    // DNS resolution failure
    if lower.contains("could not resolve") || lower.contains("dns") && lower.contains("fail") {
        return "Cannot resolve server address. Check your internet connection or try a different server.".to_string();
    }

    // Generic fallback with prefix
    format!("Connection failed: {}", raw)
}

// ── Commands ───────────────────────────────────────────

/// Get list of all servers
#[tauri::command]
pub async fn get_servers(ctx: State<'_, AppContext>) -> Result<Vec<ServerInfo>, String> {
    let state = ctx.state.lock().await;
    let servers: Vec<ServerInfo> = state.servers.iter().map(ServerInfo::from).collect();
    Ok(servers)
}

/// Import servers from links (one per line, or base64 subscription content)
#[tauri::command]
pub async fn add_links(ctx: State<'_, AppContext>, links: String) -> Result<Vec<ServerInfo>, String> {
    let servers = link_parser::parse_subscription_content(&links);

    if servers.is_empty() {
        return Err("No valid links found".to_string());
    }

    let infos: Vec<ServerInfo> = servers.iter().map(ServerInfo::from).collect();

    let mut state = ctx.state.lock().await;
    state.servers.extend(servers);
    save_state(&state);

    Ok(infos)
}

/// Add a subscription URL
#[tauri::command]
pub async fn add_subscription(
    ctx: State<'_, AppContext>,
    url: String,
    name: Option<String>,
) -> Result<Vec<ServerInfo>, String> {
    let hwid_enabled = ctx.state.lock().await.settings.hwid_enabled;
    let (sub, servers) = subscription::fetch_subscription(&url, name.as_deref(), hwid_enabled, Some(ctx.app_logs.clone()))
        .await
        .map_err(|e| format!("Failed to fetch subscription: {}", e))?;

    if servers.is_empty() {
        return Err("Subscription returned no servers".to_string());
    }

    let infos: Vec<ServerInfo> = servers.iter().map(ServerInfo::from).collect();

    let mut state = ctx.state.lock().await;
    state.servers.extend(servers);
    state.subscriptions.push(sub);
    save_state(&state);

    Ok(infos)
}

/// Remove a server by ID
#[tauri::command]
pub async fn remove_server(ctx: State<'_, AppContext>, server_id: String) -> Result<(), String> {
    let mut state = ctx.state.lock().await;
    state.servers.retain(|s| s.id != server_id);
    // Also remove from subscription server lists
    for sub in &mut state.subscriptions {
        sub.servers.retain(|id| id != &server_id);
    }
    save_state(&state);
    Ok(())
}

/// Connect to a server
#[tauri::command]
pub async fn connect(ctx: State<'_, AppContext>, server_id: String) -> Result<StatusResponse, String> {
    let mut state = ctx.state.lock().await;
    let server = state
        .servers
        .iter()
        .find(|s| s.id == server_id)
        .cloned()
        .ok_or("Server not found")?;

    let tun_mode = state.settings.vpn_mode == "tun";
    let routing_rules = state.routing_rules.clone();
    let default_route = state.default_route.clone();
    let per_app_mode = state.settings.per_app_mode.clone();
    let per_app_list = state.settings.per_app_list.clone();
    let stealth = state.settings.stealth_mode;

    // Auto port mode: generate random ports each connection to avoid fingerprinting
    if state.settings.port_mode == "auto" {
        let (socks, http) = CoreManager::generate_random_ports();
        ctx.core.set_ports(socks, http).await;
        log::info!("Auto port mode: using random ports SOCKS:{} HTTP:{}", socks, http);
    }

    ctx.core
        .start(&server, tun_mode, &routing_rules, &default_route, &per_app_mode, &per_app_list, stealth)
        .await
        .map_err(|e| humanize_core_error(&e.to_string(), tun_mode))?;

    let http_port = ctx.core.http_port().await;

    // Only set system proxy in proxy mode; TUN captures all traffic directly
    if !tun_mode {
        proxy_setter::set_system_proxy("127.0.0.1", http_port)
            .map_err(|e| format!("Failed to set system proxy: {}", e))?;
    }

    // Record session
    let record = ConnectionRecord {
        server_name: server.name.clone(),
        server_address: format!("{}:{}", server.address, server.port),
        protocol: format!("{:?}", server.protocol).to_lowercase(),
        core_type: format!("{:?}", ctx.core.get_core_type().await),
        vpn_mode: if tun_mode { "tun".to_string() } else { "proxy".to_string() },
        connected_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        disconnected_at: None,
        upload_bytes: 0,
        download_bytes: 0,
    };
    state.sessions.push(record);

    state.active_server_id = Some(server_id);
    save_state(&state);

    Ok(StatusResponse {
        connected: true,
        server_name: Some(server.name),
        core_type: format!("{:?}", ctx.core.get_core_type().await),
        socks_port: ctx.core.socks_port().await,
        http_port,
    })
}

/// Disconnect
#[tauri::command]
pub async fn disconnect(ctx: State<'_, AppContext>) -> Result<StatusResponse, String> {
    // Grab traffic stats before stopping
    let traffic = ctx.core.get_traffic_stats().await;

    if let Err(e) = proxy_setter::unset_system_proxy() {
        log::error!("Failed to unset system proxy: {}", e);
    }

    if let Err(e) = ctx.core.stop().await {
        log::error!("Failed to stop core: {}", e);
    }

    let mut state = ctx.state.lock().await;

    // Finalize last open session
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    if let Some(last) = state.sessions.last_mut() {
        if last.disconnected_at.is_none() {
            last.disconnected_at = Some(now);
            last.upload_bytes = traffic.upload;
            last.download_bytes = traffic.download;
        }
    }
    // Keep last 50 sessions
    let slen = state.sessions.len();
    if slen > 50 {
        state.sessions.drain(0..slen - 50);
    }

    state.active_server_id = None;
    save_state(&state);

    Ok(StatusResponse {
        connected: false,
        server_name: None,
        core_type: format!("{:?}", ctx.core.get_core_type().await),
        socks_port: ctx.core.socks_port().await,
        http_port: ctx.core.http_port().await,
    })
}

/// Get connection status
#[tauri::command]
pub async fn get_status(ctx: State<'_, AppContext>) -> Result<StatusResponse, String> {
    let running = ctx.core.is_running().await;
    let state = ctx.state.lock().await;

    let server_name = if running {
        state.active_server_id.as_ref().and_then(|id| {
            state.servers.iter().find(|s| &s.id == id).map(|s| s.name.clone())
        })
    } else {
        None
    };

    Ok(StatusResponse {
        connected: running,
        server_name,
        core_type: format!("{:?}", ctx.core.get_core_type().await),
        socks_port: ctx.core.socks_port().await,
        http_port: ctx.core.http_port().await,
    })
}

/// Switch between sing-box and xray-core
#[tauri::command]
pub async fn set_core_type(ctx: State<'_, AppContext>, core: String) -> Result<String, String> {
    let core_type = match core.to_lowercase().as_str() {
        "singbox" | "sing-box" => CoreType::SingBox,
        "xray" | "xray-core" => CoreType::Xray,
        _ => return Err(format!("Unknown core: {}", core)),
    };

    let was_running = ctx.core.is_running().await;
    if was_running {
        ctx.core.stop().await.map_err(|e| e.to_string())?;
    }

    ctx.core.set_core_type(core_type.clone()).await;

    let mut state = ctx.state.lock().await;
    state.selected_core = core_type.clone();
    save_state(&state);

    if was_running {
        if let Some(id) = &state.active_server_id {
            if let Some(server) = state.servers.iter().find(|s| &s.id == id) {
                let s = server.clone();
                let tun_mode = state.settings.vpn_mode == "tun";
                let rules = state.routing_rules.clone();
                let dr = state.default_route.clone();
                let pam = state.settings.per_app_mode.clone();
                let pal = state.settings.per_app_list.clone();
                let stl = state.settings.stealth_mode;
                drop(state);
                ctx.core.start(&s, tun_mode, &rules, &dr, &pam, &pal, stl).await.map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(format!("{:?}", core_type))
}

/// Ping a single server
#[tauri::command]
pub async fn ping_server(ctx: State<'_, AppContext>, server_id: String) -> Result<Option<u32>, String> {
    let state = ctx.state.lock().await;
    let server = state
        .servers
        .iter()
        .find(|s| s.id == server_id)
        .ok_or("Server not found")?;

    let addr = server.address.clone();
    let port = server.port;
    drop(state);

    let result = ping::ping_average(&addr, port).await;

    let mut state = ctx.state.lock().await;
    if let Some(s) = state.servers.iter_mut().find(|s| s.id == server_id) {
        s.latency_ms = result;
    }
    save_state(&state);

    Ok(result)
}

/// Ping all servers
#[tauri::command]
pub async fn ping_all_servers(ctx: State<'_, AppContext>) -> Result<Vec<(String, Option<u32>)>, String> {
    let state = ctx.state.lock().await;
    let targets: Vec<(String, String, u16)> = state
        .servers
        .iter()
        .map(|s| (s.id.clone(), s.address.clone(), s.port))
        .collect();
    drop(state);

    let results = ping::ping_all(&targets).await;

    let mut state = ctx.state.lock().await;
    for (id, latency) in &results {
        if let Some(s) = state.servers.iter_mut().find(|s| &s.id == id) {
            s.latency_ms = *latency;
        }
    }
    save_state(&state);

    Ok(results)
}

// ── New commands ──────────────────────────────────────

/// Get list of subscriptions with meta info
#[tauri::command]
pub async fn get_subscriptions(ctx: State<'_, AppContext>) -> Result<Vec<SubscriptionInfo>, String> {
    let state = ctx.state.lock().await;
    let subs: Vec<SubscriptionInfo> = state
        .subscriptions
        .iter()
        .map(|sub| SubscriptionInfo::from_sub(sub, &state.servers))
        .collect();
    Ok(subs)
}

/// Update (re-fetch) a subscription by ID
#[tauri::command]
pub async fn update_subscription(ctx: State<'_, AppContext>, subscription_id: String) -> Result<Vec<ServerInfo>, String> {
    let state = ctx.state.lock().await;
    let sub = state
        .subscriptions
        .iter()
        .find(|s| s.id == subscription_id)
        .cloned()
        .ok_or("Subscription not found")?;
    let hwid_enabled = state.settings.hwid_enabled;
    drop(state);

    let (new_sub, new_servers) = subscription::fetch_subscription(&sub.url, Some(&sub.name), hwid_enabled, Some(ctx.app_logs.clone()))
        .await
        .map_err(|e| format!("Failed to update subscription: {}", e))?;

    let mut state = ctx.state.lock().await;

    // Remove old servers belonging to this subscription
    state.servers.retain(|s| s.subscription_id.as_deref() != Some(&subscription_id));

    // Tag new servers with the existing subscription ID
    let mut tagged_servers = new_servers;
    for server in &mut tagged_servers {
        server.subscription_id = Some(subscription_id.clone());
    }

    let infos: Vec<ServerInfo> = tagged_servers.iter().map(ServerInfo::from).collect();
    let server_ids: Vec<String> = tagged_servers.iter().map(|s| s.id.clone()).collect();

    state.servers.extend(tagged_servers);

    // Update subscription metadata
    if let Some(existing) = state.subscriptions.iter_mut().find(|s| s.id == subscription_id) {
        existing.servers = server_ids;
        existing.updated_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
        existing.name = new_sub.name;
        existing.update_interval = new_sub.update_interval;
        existing.upload = new_sub.upload;
        existing.download = new_sub.download;
        existing.total = new_sub.total;
        existing.expire = new_sub.expire;
        existing.web_page_url = new_sub.web_page_url;
        existing.support_url = new_sub.support_url;
        existing.announce = new_sub.announce;
        existing.refill_date = new_sub.refill_date;
    }

    save_state(&state);
    Ok(infos)
}

/// Delete a subscription and all its servers
#[tauri::command]
pub async fn delete_subscription(ctx: State<'_, AppContext>, subscription_id: String) -> Result<(), String> {
    let mut state = ctx.state.lock().await;
    state.servers.retain(|s| s.subscription_id.as_deref() != Some(&subscription_id));
    state.subscriptions.retain(|s| s.id != subscription_id);
    save_state(&state);
    Ok(())
}

/// Auto-select the best server (lowest ping)
#[tauri::command]
pub async fn auto_select_server(ctx: State<'_, AppContext>) -> Result<ServerInfo, String> {
    let state = ctx.state.lock().await;
    if state.servers.is_empty() {
        return Err("No servers available".to_string());
    }

    let targets: Vec<(String, String, u16)> = state
        .servers
        .iter()
        .map(|s| (s.id.clone(), s.address.clone(), s.port))
        .collect();
    drop(state);

    let results = ping::ping_all(&targets).await;

    // Find server with lowest latency
    let best = results
        .iter()
        .filter_map(|(id, ms)| ms.map(|ms| (id, ms)))
        .min_by_key(|(_, ms)| *ms);

    let best_id = best
        .map(|(id, _)| id.clone())
        .ok_or("No reachable servers found")?;

    // Update all latencies
    let mut state = ctx.state.lock().await;
    for (id, latency) in &results {
        if let Some(s) = state.servers.iter_mut().find(|s| &s.id == id) {
            s.latency_ms = *latency;
        }
    }
    save_state(&state);

    let server = state
        .servers
        .iter()
        .find(|s| s.id == best_id)
        .ok_or("Server not found")?;

    Ok(ServerInfo::from(server))
}

/// Export config as JSON string
#[tauri::command]
pub async fn export_config(ctx: State<'_, AppContext>) -> Result<String, String> {
    let state = ctx.state.lock().await;
    serde_json::to_string_pretty(&*state).map_err(|e| e.to_string())
}

/// Import config from JSON string
#[tauri::command]
pub async fn import_config(ctx: State<'_, AppContext>, data: String) -> Result<String, String> {
    let imported: AppState = serde_json::from_str(&data)
        .map_err(|e| format!("Invalid config format: {}", e))?;

    let mut state = ctx.state.lock().await;

    let added_servers = imported.servers.len();
    let added_subs = imported.subscriptions.len();

    // Merge: add imported servers and subscriptions (avoid duplicates by ID)
    for server in imported.servers {
        if !state.servers.iter().any(|s| s.id == server.id) {
            state.servers.push(server);
        }
    }
    for sub in imported.subscriptions {
        if !state.subscriptions.iter().any(|s| s.id == sub.id) {
            state.subscriptions.push(sub);
        }
    }

    save_state(&state);
    Ok(format!("Imported {} servers, {} subscriptions", added_servers, added_subs))
}

/// Get traffic statistics (always succeeds — returns zeros if core isn't running or stats unavailable)
#[tauri::command]
pub async fn get_traffic_stats(ctx: State<'_, AppContext>) -> Result<TrafficStats, String> {
    if !ctx.core.is_running().await {
        return Ok(TrafficStats::default());
    }

    Ok(ctx.core.get_traffic_stats().await)
}

/// Get current settings
#[tauri::command]
pub async fn get_settings(ctx: State<'_, AppContext>) -> Result<Settings, String> {
    let state = ctx.state.lock().await;
    Ok(state.settings.clone())
}

/// Save settings
#[tauri::command]
pub async fn save_settings(ctx: State<'_, AppContext>, settings: Settings) -> Result<(), String> {
    let old_socks;
    let old_http;

    {
        let mut state = ctx.state.lock().await;
        old_socks = state.settings.socks_port;
        old_http = state.settings.http_port;
        state.settings = settings.clone();
        save_state(&state);
    }

    // If ports changed and in manual mode, update CoreManager and reconnect if needed
    let ports_changed = settings.port_mode == "manual" && (old_socks != settings.socks_port || old_http != settings.http_port);
    if ports_changed {
        ctx.core.set_ports(settings.socks_port, settings.http_port).await;

        if ctx.core.is_running().await {
            let state = ctx.state.lock().await;
            if let Some(id) = &state.active_server_id {
                if let Some(server) = state.servers.iter().find(|s| &s.id == id) {
                    let s = server.clone();
                    let tun_mode = state.settings.vpn_mode == "tun";
                    let rules = state.routing_rules.clone();
                    let dr = state.default_route.clone();
                    let pam = state.settings.per_app_mode.clone();
                    let pal = state.settings.per_app_list.clone();
                    let stl = state.settings.stealth_mode;
                    drop(state);
                    // Reconnect with new ports
                    if let Err(e) = ctx.core.start(&s, tun_mode, &rules, &dr, &pam, &pal, stl).await {
                        log::error!("Failed to reconnect with new ports: {}", e);
                    }
                    // Update system proxy with current HTTP port (only in proxy mode)
                    if !tun_mode {
                        let current_http = ctx.core.http_port().await;
                        if let Err(e) = proxy_setter::set_system_proxy("127.0.0.1", current_http) {
                            log::error!("Failed to update system proxy: {}", e);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Get core process logs
#[tauri::command]
pub async fn get_logs(ctx: State<'_, AppContext>) -> Result<Vec<String>, String> {
    Ok(ctx.core.get_logs().await)
}

/// Clear core process logs
#[tauri::command]
pub async fn clear_logs(ctx: State<'_, AppContext>) -> Result<(), String> {
    ctx.core.clear_logs().await;
    Ok(())
}

/// Get app-level logs
#[tauri::command]
pub async fn get_app_logs(ctx: State<'_, AppContext>) -> Result<Vec<String>, String> {
    Ok(ctx.app_logs.lock().await.clone())
}

/// Clear app-level logs
#[tauri::command]
pub async fn clear_app_logs(ctx: State<'_, AppContext>) -> Result<(), String> {
    ctx.app_logs.lock().await.clear();
    Ok(())
}

/// Get connection history (last 50 sessions)
#[tauri::command]
pub async fn get_connection_history(ctx: State<'_, AppContext>) -> Result<Vec<ConnectionRecord>, String> {
    let state = ctx.state.lock().await;
    Ok(state.sessions.clone())
}

/// Clear connection history
#[tauri::command]
pub async fn clear_connection_history(ctx: State<'_, AppContext>) -> Result<(), String> {
    let mut state = ctx.state.lock().await;
    state.sessions.clear();
    save_state(&state);
    Ok(())
}

/// Get device HWID info
#[tauri::command]
pub fn get_device_info() -> hwid::DeviceInfo {
    hwid::get_device_info()
}

/// Open a URL in the system browser
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        // Write URL to file for MainActivity to pick up via intent
        for base in &[
            "/data/data/com.horusvpn.nexvpn/files",
            "/data/user/0/com.horusvpn.nexvpn/files",
        ] {
            let path = std::path::PathBuf::from(base).join("nexvpn/.open_url");
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            if std::fs::write(&path, &url).is_ok() {
                log::info!("open_url: wrote {} to {}", url, path.display());
                return Ok(());
            }
        }
        Err("Failed to write URL file".into())
    }

    #[cfg(not(target_os = "android"))]
    {
        // Desktop: use std::process::Command to open URL
        #[cfg(target_os = "windows")]
        { std::process::Command::new("cmd").args(["/c", "start", "", &url]).spawn().map_err(|e| e.to_string())?; }
        #[cfg(target_os = "macos")]
        { std::process::Command::new("open").arg(&url).spawn().map_err(|e| e.to_string())?; }
        #[cfg(target_os = "linux")]
        { std::process::Command::new("xdg-open").arg(&url).spawn().map_err(|e| e.to_string())?; }
        Ok(())
    }
}

// ── Persistence ────────────────────────────────────────

fn state_path() -> std::path::PathBuf {
    #[cfg(target_os = "android")]
    {
        for base in &[
            "/data/user/0/com.horusvpn.nexvpn/files",
            "/data/data/com.horusvpn.nexvpn/files",
        ] {
            let dir = std::path::PathBuf::from(base).join("nexvpn");
            if std::fs::create_dir_all(&dir).is_ok() {
                return dir.join("state.json");
            }
        }
        return std::path::PathBuf::from("/data/data/com.horusvpn.nexvpn/files/nexvpn/state.json");
    }

    #[cfg(not(target_os = "android"))]
    {
        dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("nexvpn")
            .join("state.json")
    }
}

// ── Routing rules ──────────────────────────────────────

#[derive(Serialize)]
pub struct RoutingRulesResponse {
    pub rules: Vec<RoutingRule>,
    pub default_route: String,
}

#[tauri::command]
pub async fn get_routing_rules(ctx: State<'_, AppContext>) -> Result<RoutingRulesResponse, String> {
    let state = ctx.state.lock().await;
    Ok(RoutingRulesResponse {
        rules: state.routing_rules.clone(),
        default_route: state.default_route.clone(),
    })
}

#[tauri::command]
pub async fn save_routing_rules(
    rules: Vec<RoutingRule>,
    default_route: String,
    ctx: State<'_, AppContext>,
) -> Result<(), String> {
    let mut state = ctx.state.lock().await;
    state.routing_rules = rules;
    state.default_route = default_route;
    save_state(&state);

    // If connected, reconnect to apply new rules
    if ctx.core.is_running().await {
        if let Some(id) = &state.active_server_id {
            if let Some(server) = state.servers.iter().find(|s| &s.id == id) {
                let s = server.clone();
                let tun_mode = state.settings.vpn_mode == "tun";
                let rules = state.routing_rules.clone();
                let dr = state.default_route.clone();
                let pam = state.settings.per_app_mode.clone();
                let pal = state.settings.per_app_list.clone();
                let stl = state.settings.stealth_mode;
                drop(state);
                if let Err(e) = ctx.core.start(&s, tun_mode, &rules, &dr, &pam, &pal, stl).await {
                    log::error!("Failed to reconnect with new routing rules: {}", e);
                }
            }
        }
    }

    Ok(())
}

/// Check if onboarding is completed
#[tauri::command]
pub async fn get_onboarding_completed(ctx: State<'_, AppContext>) -> Result<bool, String> {
    let state = ctx.state.lock().await;
    Ok(state.onboarding_completed)
}

/// Mark onboarding as completed
#[tauri::command]
pub async fn complete_onboarding(ctx: State<'_, AppContext>) -> Result<(), String> {
    let mut state = ctx.state.lock().await;
    state.onboarding_completed = true;
    save_state(&state);
    Ok(())
}

/// Check if the process is running with elevated (admin) privileges
#[tauri::command]
pub fn is_admin() -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // Check via "net session" — only succeeds when running as admin
        std::process::Command::new("net")
            .args(["session"])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(target_os = "macos")]
    {
        // On macOS check effective UID via id -u
        std::process::Command::new("id")
            .arg("-u")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
            .unwrap_or(false)
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("id")
            .arg("-u")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
            .unwrap_or(false)
    }
    #[cfg(target_os = "android")]
    {
        // Android handles VPN through VpnService, no admin needed
        true
    }
}

/// Restart the application with elevated (admin) privileges
#[tauri::command]
pub fn restart_as_admin(app_handle: tauri::AppHandle) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // Use ShellExecuteW via powershell Start-Process -Verb RunAs
        std::process::Command::new("powershell")
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .args([
                "-Command",
                &format!(
                    "Start-Process '{}' -Verb RunAs",
                    exe.display()
                ),
            ])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        // Use osascript to prompt for admin
        std::process::Command::new("osascript")
            .args([
                "-e",
                &format!(
                    "do shell script \"'{}' &\" with administrator privileges",
                    exe.display()
                ),
            ])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        // Try pkexec
        std::process::Command::new("pkexec")
            .arg(exe)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    // Close current instance
    app_handle.exit(0);
    Ok(())
}

/// Check current public IP address
#[tauri::command]
pub async fn check_ip() -> Result<IpCheckResult, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    // Try ip-api.com for IP + geo info
    let resp = client
        .get("http://ip-api.com/json/?fields=query,country,city,lat,lon")
        .send()
        .await
        .map_err(|e| format!("Failed to check IP: {}", e))?;

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    Ok(IpCheckResult {
        ip: body["query"].as_str().unwrap_or("unknown").to_string(),
        country: body["country"].as_str().unwrap_or("unknown").to_string(),
        city: body["city"].as_str().unwrap_or("unknown").to_string(),
        lat: body["lat"].as_f64().unwrap_or(0.0),
        lon: body["lon"].as_f64().unwrap_or(0.0),
    })
}

/// Simple DNS leak test
#[tauri::command]
pub async fn check_dns_leak() -> Result<DnsLeakResult, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    // Use a simple approach: check if we can detect DNS resolver IPs
    // by querying a service that returns the resolver IP
    let resp = client
        .get("https://1.1.1.1/cdn-cgi/trace")
        .send()
        .await
        .map_err(|e| format!("DNS leak check failed: {}", e))?;

    let text = resp.text().await.map_err(|e| e.to_string())?;

    let mut ip = String::new();
    for line in text.lines() {
        if line.starts_with("ip=") {
            ip = line.trim_start_matches("ip=").to_string();
        }
    }

    Ok(DnsLeakResult {
        leaked: false, // simplified - compare with VPN IP in frontend
        dns_servers: if ip.is_empty() { vec![] } else { vec![ip] },
    })
}

/// Run a basic download speed test
#[tauri::command]
pub async fn run_speed_test() -> Result<SpeedTestResult, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    // Ping test
    let ping_start = std::time::Instant::now();
    let _ = client.head("https://speed.cloudflare.com").send().await;
    let ping_ms = ping_start.elapsed().as_millis() as u32;

    // Download test - fetch ~10MB from Cloudflare
    let dl_start = std::time::Instant::now();
    let resp = client
        .get("https://speed.cloudflare.com/__down?bytes=10000000")
        .send()
        .await
        .map_err(|e| format!("Download test failed: {}", e))?;

    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    let dl_duration = dl_start.elapsed().as_secs_f64();
    let dl_mbps = if dl_duration > 0.0 {
        (bytes.len() as f64 * 8.0) / (dl_duration * 1_000_000.0)
    } else {
        0.0
    };

    // Upload test - send ~2MB to Cloudflare
    let upload_data = vec![0u8; 2_000_000];
    let ul_start = std::time::Instant::now();
    let _ = client
        .post("https://speed.cloudflare.com/__up")
        .body(upload_data.clone())
        .send()
        .await;
    let ul_duration = ul_start.elapsed().as_secs_f64();
    let ul_mbps = if ul_duration > 0.0 {
        (upload_data.len() as f64 * 8.0) / (ul_duration * 1_000_000.0)
    } else {
        0.0
    };

    Ok(SpeedTestResult {
        download_mbps: (dl_mbps * 100.0).round() / 100.0,
        upload_mbps: (ul_mbps * 100.0).round() / 100.0,
        ping_ms,
    })
}

/// Compare two semver strings: returns true if a > b
fn version_gt(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.split('.').filter_map(|p| p.parse().ok()).collect()
    };
    let va = parse(a);
    let vb = parse(b);
    for i in 0..va.len().max(vb.len()) {
        let na = va.get(i).copied().unwrap_or(0);
        let nb = vb.get(i).copied().unwrap_or(0);
        if na > nb { return true; }
        if na < nb { return false; }
    }
    false
}

/// Check for updates on GitHub
#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateCheckResult, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("NexVPN")
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get("https://api.github.com/repos/TopPro104/nexvpn/releases/latest")
        .send()
        .await
        .map_err(|e| format!("Failed to check updates: {}", e))?;

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    let tag = body["tag_name"].as_str().unwrap_or("");
    let latest = tag.trim_start_matches('v').to_string();
    let current = env!("CARGO_PKG_VERSION").to_string();
    let changelog = body["body"].as_str().unwrap_or("").to_string();
    let download_url = body["html_url"].as_str().unwrap_or("").to_string();

    // Only show update if latest version is strictly newer than current
    let has_update = !tag.is_empty()
        && !latest.is_empty()
        && latest != "0.0.0"
        && version_gt(&latest, &current);

    Ok(UpdateCheckResult {
        has_update,
        latest_version: latest,
        current_version: current,
        changelog,
        download_url,
    })
}

/// Toggle server favorite status
#[tauri::command]
pub async fn toggle_favorite(ctx: State<'_, AppContext>, server_id: String) -> Result<bool, String> {
    let mut state = ctx.state.lock().await;
    let server = state.servers.iter_mut()
        .find(|s| s.id == server_id)
        .ok_or("Server not found")?;
    server.favorite = !server.favorite;
    let new_status = server.favorite;
    save_state(&state);
    Ok(new_status)
}

/// Get daily traffic aggregated from connection history
#[tauri::command]
pub async fn get_daily_traffic(ctx: State<'_, AppContext>) -> Result<Vec<DailyTraffic>, String> {
    let state = ctx.state.lock().await;
    let mut daily: std::collections::HashMap<String, (u64, u64)> = std::collections::HashMap::new();

    for session in &state.sessions {
        let date = format_epoch_date(session.connected_at);
        let entry = daily.entry(date).or_insert((0, 0));
        entry.0 += session.upload_bytes;
        entry.1 += session.download_bytes;
    }

    let mut result: Vec<DailyTraffic> = daily.into_iter()
        .map(|(date, (upload, download))| DailyTraffic { date, upload, download })
        .collect();
    result.sort_by(|a, b| a.date.cmp(&b.date));

    // Keep last 7 days
    if result.len() > 7 {
        result = result.split_off(result.len() - 7);
    }

    Ok(result)
}

/// Get server usage statistics
#[tauri::command]
pub async fn get_server_usage_stats(ctx: State<'_, AppContext>) -> Result<Vec<ServerUsageStat>, String> {
    let state = ctx.state.lock().await;
    let mut stats: std::collections::HashMap<String, (String, u32, u64)> = std::collections::HashMap::new();

    for session in &state.sessions {
        let entry = stats.entry(session.server_name.clone())
            .or_insert((session.protocol.clone(), 0, 0));
        entry.1 += 1;
        entry.2 += session.upload_bytes + session.download_bytes;
    }

    let mut result: Vec<ServerUsageStat> = stats.into_iter()
        .map(|(name, (protocol, count, traffic))| ServerUsageStat {
            server_name: name,
            protocol,
            connection_count: count,
            total_traffic: traffic,
        })
        .collect();

    result.sort_by(|a, b| b.total_traffic.cmp(&a.total_traffic));
    result.truncate(10);

    Ok(result)
}

/// Get the last active server ID (persisted across restarts)
#[tauri::command]
pub async fn get_active_server_id(ctx: State<'_, AppContext>) -> Result<Option<String>, String> {
    let state = ctx.state.lock().await;
    Ok(state.active_server_id.clone())
}

/// Save the selected server ID (persisted across restarts, even without connecting)
#[tauri::command]
pub async fn set_selected_server(ctx: State<'_, AppContext>, server_id: Option<String>) -> Result<(), String> {
    let mut state = ctx.state.lock().await;
    state.active_server_id = server_id;
    save_state(&state);
    Ok(())
}

/// Read and clear tile action file (Android Quick Settings Tile)
#[tauri::command]
pub fn read_tile_action() -> Option<String> {
    #[cfg(target_os = "android")]
    {
        for base in &[
            "/data/data/com.horusvpn.nexvpn/files",
            "/data/user/0/com.horusvpn.nexvpn/files",
        ] {
            let path = std::path::PathBuf::from(base).join("nexvpn/.tile_action");
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let action = content.trim().to_string();
                    std::fs::remove_file(&path).ok();
                    if !action.is_empty() {
                        log::info!("read_tile_action: {}", action);
                        return Some(action);
                    }
                }
            }
        }
        None
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

fn format_epoch_date(epoch_secs: u64) -> String {
    // Simple date formatting: YYYY-MM-DD from epoch seconds
    // This is approximate but good enough for daily grouping
    let days_since_epoch = epoch_secs / 86400;
    let mut y = 1970i64;
    let mut remaining = days_since_epoch as i64;

    loop {
        let days_in_year = if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }

    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md as i64 {
            m = i + 1;
            break;
        }
        remaining -= md as i64;
    }
    if m == 0 { m = 12; }
    let d = remaining + 1;

    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Get Xposed module status (Android only)
#[derive(Serialize)]
pub struct XposedStatus {
    pub active: bool,
    pub hooked_apps: Vec<String>,
}

#[tauri::command]
pub fn get_xposed_status() -> XposedStatus {
    #[cfg(target_os = "android")]
    {
        // Check 1: status file written by Xposed module from hooked apps
        for base in &[
            "/data/data/com.horusvpn.nexvpn/files",
            "/data/user/0/com.horusvpn.nexvpn/files",
        ] {
            let path = std::path::PathBuf::from(base).join("nexvpn/.xposed_status");
            if let Ok(content) = std::fs::read_to_string(&path) {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let apps: Vec<String> = content.lines()
                    .filter_map(|line| {
                        let (pkg, ts) = line.split_once(':')?;
                        let ts: u64 = ts.parse().ok()?;
                        if now - ts < 86_400_000 { Some(pkg.to_string()) } else { None }
                    })
                    .collect();
                if !apps.is_empty() {
                    return XposedStatus { active: true, hooked_apps: apps };
                }
            }
        }

        // Check 2: use root to read LSPosed module scope
        if let Ok(output) = std::process::Command::new("su")
            .args(["-c", "ls /data/adb/lsposed/config/ 2>/dev/null && cat /data/adb/lsposed/config/modules/com.horusvpn.nexvpn/scope.list 2>/dev/null || echo ''"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            if text.contains("modules") {
                // LSPosed exists
                let scope_apps: Vec<String> = text.lines()
                    .filter(|l| !l.is_empty() && !l.contains("modules") && !l.contains("config"))
                    .map(|l| l.trim().to_string())
                    .collect();
                if !scope_apps.is_empty() {
                    return XposedStatus { active: true, hooked_apps: scope_apps };
                }
                return XposedStatus { active: false, hooked_apps: vec!["lsposed_detected".to_string()] };
            }
        }

        // Check 3: fallback — check if lsposed dir exists via root
        if let Ok(output) = std::process::Command::new("su")
            .args(["-c", "test -d /data/adb/lsposed && echo yes || echo no"])
            .output()
        {
            if String::from_utf8_lossy(&output.stdout).contains("yes") {
                return XposedStatus { active: false, hooked_apps: vec!["lsposed_detected".to_string()] };
            }
        }
    }
    XposedStatus { active: false, hooked_apps: vec![] }
}

/// Get shareable link for a server
#[tauri::command]
pub async fn get_server_link(ctx: State<'_, AppContext>, server_id: String) -> Result<String, String> {
    let state = ctx.state.lock().await;
    let server = state.servers.iter()
        .find(|s| s.id == server_id)
        .ok_or("Server not found")?;
    Ok(link_parser::server_to_link(server))
}

/// Get installed apps (Android only, for per-app VPN)
#[derive(Serialize, serde::Deserialize)]
pub struct InstalledApp {
    pub package_name: String,
    pub label: String,
    #[serde(default)]
    pub icon: String, // base64 PNG
}

#[tauri::command]
pub fn get_installed_apps() -> Vec<InstalledApp> {
    #[cfg(target_os = "android")]
    {
        // Read JSON written by MainActivity (includes labels + base64 icons)
        for base in &[
            "/data/data/com.horusvpn.nexvpn/files",
            "/data/user/0/com.horusvpn.nexvpn/files",
        ] {
            let path = std::path::PathBuf::from(base).join("nexvpn/.installed_apps.json");
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(apps) = serde_json::from_str::<Vec<InstalledApp>>(&content) {
                    return apps;
                }
            }
        }
        vec![]
    }
    #[cfg(not(target_os = "android"))]
    {
        vec![]
    }
}

/// Measure real latency through VPN tunnel (SOCKS5 proxy → external endpoint)
#[tauri::command]
pub async fn ping_through_vpn(ctx: State<'_, AppContext>) -> Result<u32, String> {
    if !ctx.core.is_running().await {
        return Err("VPN not connected".into());
    }

    let socks_port = ctx.core.socks_port().await;
    let (user, pass) = ctx.core.proxy_auth();

    let proxy_url = format!("socks5://{}:{}@127.0.0.1:{}", user, pass, socks_port);
    let proxy = reqwest::Proxy::all(&proxy_url).map_err(|e| e.to_string())?;

    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let start = std::time::Instant::now();
    client.head("https://1.1.1.1")
        .send()
        .await
        .map_err(|e| format!("Ping failed: {}", e))?;

    Ok(start.elapsed().as_millis() as u32)
}

pub fn load_state() -> AppState {
    let path = state_path();
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => AppState::default(),
        }
    } else {
        AppState::default()
    }
}

fn save_state(state: &AppState) {
    let path = state_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Ok(content) = serde_json::to_string_pretty(state) {
        std::fs::write(&path, content).ok();
    }
}
