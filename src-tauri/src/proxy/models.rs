use serde::{Deserialize, Serialize};

/// Supported proxy protocols
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Vless,
    Vmess,
    Shadowsocks,
    Trojan,
    Hysteria2,
    Tuic,
}

/// Transport type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Tcp,
    Ws,
    Grpc,
    Http,
    Quic,
}

/// TLS settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TlsSettings {
    pub enabled: bool,
    pub server_name: Option<String>,
    pub insecure: bool,
    pub alpn: Vec<String>,
    pub fingerprint: Option<String>,
    pub reality: Option<RealitySettings>,
}

/// VLESS Reality settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealitySettings {
    pub public_key: String,
    pub short_id: String,
}

/// WebSocket settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WsSettings {
    pub path: String,
    pub host: Option<String>,
}

/// gRPC settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GrpcSettings {
    pub service_name: String,
}

/// A single proxy server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub protocol: Protocol,

    // Protocol-specific
    pub uuid: Option<String>,       // vless, vmess
    pub password: Option<String>,   // ss, trojan
    pub method: Option<String>,     // ss encryption method
    pub flow: Option<String>,       // vless flow (xtls-rprx-vision)
    pub alter_id: Option<u32>,      // vmess

    // Transport
    pub transport: Transport,
    pub ws: Option<WsSettings>,
    pub grpc: Option<GrpcSettings>,

    // TLS
    pub tls: TlsSettings,

    // Metadata
    pub subscription_id: Option<String>,
    pub latency_ms: Option<u32>,
}

/// A subscription source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub url: String,
    pub servers: Vec<String>, // server IDs
    pub updated_at: Option<u64>,
}

/// User settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub theme: String,
    #[serde(default = "default_style")]
    pub style: String,
    pub socks_port: u16,
    pub http_port: u16,
    pub auto_connect: bool,
    pub language: String,
    #[serde(default = "default_vpn_mode")]
    pub vpn_mode: String, // "proxy" or "tun"
    #[serde(default)]
    pub auto_reconnect: bool,
    #[serde(default = "default_true")]
    pub hwid_enabled: bool,
    #[serde(default = "default_animation")]
    pub animation: String,
}

fn default_style() -> String { "default".to_string() }
fn default_vpn_mode() -> String { "proxy".to_string() }
fn default_true() -> bool { true }
fn default_animation() -> String { "smooth".to_string() }

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            style: "default".to_string(),
            socks_port: 10808,
            http_port: 10809,
            auto_connect: false,
            language: "en".to_string(),
            vpn_mode: "proxy".to_string(),
            auto_reconnect: false,
            hwid_enabled: true,
            animation: "smooth".to_string(),
        }
    }
}

/// Action for a routing rule
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    #[default]
    Proxy,
    Direct,
    Block,
}

/// A user-defined routing rule (domain → action)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    pub id: String,
    pub domain: String,
    pub action: RuleAction,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// A recorded connection session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionRecord {
    pub server_name: String,
    pub server_address: String,
    pub protocol: String,
    pub core_type: String,
    pub vpn_mode: String,
    pub connected_at: u64,
    pub disconnected_at: Option<u64>,
    pub upload_bytes: u64,
    pub download_bytes: u64,
}

/// App state persisted to disk
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    pub servers: Vec<Server>,
    pub subscriptions: Vec<Subscription>,
    pub active_server_id: Option<String>,
    pub selected_core: CoreType,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub sessions: Vec<ConnectionRecord>,
    #[serde(default)]
    pub routing_rules: Vec<RoutingRule>,
    #[serde(default = "default_route")]
    pub default_route: String,
}

fn default_route() -> String { "proxy".to_string() }

/// Which proxy core to use
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum CoreType {
    #[default]
    SingBox,
    Xray,
}

/// Traffic statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrafficStats {
    pub upload: u64,
    pub download: u64,
}
