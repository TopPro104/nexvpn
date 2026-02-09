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
        }
    }
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
    let (sub, servers) = subscription::fetch_subscription(&url, name.as_deref(), hwid_enabled)
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

    ctx.core
        .start(&server, tun_mode, &routing_rules, &default_route)
        .await
        .map_err(|e| format!("Failed to start core: {}", e))?;

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
                drop(state);
                ctx.core.start(&s, tun_mode, &rules, &dr).await.map_err(|e| e.to_string())?;
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

    let (new_sub, new_servers) = subscription::fetch_subscription(&sub.url, Some(&sub.name), hwid_enabled)
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

    // If ports changed, update CoreManager and reconnect if needed
    let ports_changed = old_socks != settings.socks_port || old_http != settings.http_port;
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
                    drop(state);
                    // Reconnect with new ports
                    if let Err(e) = ctx.core.start(&s, tun_mode, &rules, &dr).await {
                        log::error!("Failed to reconnect with new ports: {}", e);
                    }
                    // Update system proxy with new HTTP port (only in proxy mode)
                    if !tun_mode {
                        if let Err(e) = proxy_setter::set_system_proxy("127.0.0.1", settings.http_port) {
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
                drop(state);
                if let Err(e) = ctx.core.start(&s, tun_mode, &rules, &dr).await {
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
        // Check via "net session" — only succeeds when running as admin
        std::process::Command::new("net")
            .args(["session"])
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
