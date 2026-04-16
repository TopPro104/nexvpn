use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine};
use url::Url;
use uuid::Uuid;

use super::models::*;

/// Parse a single proxy link into a Server
pub fn parse_link(link: &str) -> Result<Server> {
    let link = link.trim();
    if link.starts_with("vless://") {
        parse_vless(link)
    } else if link.starts_with("vmess://") {
        parse_vmess(link)
    } else if link.starts_with("ss://") {
        parse_shadowsocks(link)
    } else if link.starts_with("trojan://") {
        parse_trojan(link)
    } else if link.starts_with("hy2://") || link.starts_with("hysteria2://") {
        parse_hysteria2(link)
    } else {
        Err(anyhow!("Unsupported link format: {}", &link[..20.min(link.len())]))
    }
}

/// Parse multiple links (one per line)
pub fn parse_links(text: &str) -> Vec<Server> {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| parse_link(l).ok())
        .collect()
}

/// Decode base64 subscription content to plain text
pub fn decode_subscription_content(content: &str) -> String {
    let decoded = general_purpose::STANDARD
        .decode(content.trim())
        .or_else(|_| general_purpose::URL_SAFE.decode(content.trim()))
        .or_else(|_| general_purpose::URL_SAFE_NO_PAD.decode(content.trim()));
    match decoded {
        Ok(bytes) => String::from_utf8(bytes).unwrap_or_else(|_| content.to_string()),
        Err(_) => content.to_string(),
    }
}

/// Decode base64 subscription content, then parse links
pub fn parse_subscription_content(content: &str) -> Vec<Server> {
    // Try base64 decode first
    let decoded = general_purpose::STANDARD
        .decode(content.trim())
        .or_else(|_| general_purpose::URL_SAFE.decode(content.trim()))
        .or_else(|_| general_purpose::URL_SAFE_NO_PAD.decode(content.trim()));

    match decoded {
        Ok(bytes) => {
            if let Ok(text) = String::from_utf8(bytes) {
                parse_links(&text)
            } else {
                parse_links(content)
            }
        }
        Err(_) => parse_links(content),
    }
}

// ── VLESS ──────────────────────────────────────────────

fn parse_vless(link: &str) -> Result<Server> {
    // vless://uuid@host:port?params#name
    let url = Url::parse(link)?;

    let uuid = url.username().to_string();
    let host = url.host_str().ok_or(anyhow!("No host"))?.to_string();
    let port = url.port().unwrap_or(443);
    let name = percent_decode(url.fragment().unwrap_or("VLESS Server"));

    let params = QueryParams::from_url(&url);

    let transport = match params.get("type").as_deref() {
        Some("ws") => Transport::Ws,
        Some("grpc") => Transport::Grpc,
        Some("http") => Transport::Http,
        Some("quic") => Transport::Quic,
        Some("xhttp") | Some("splithttp") => Transport::Xhttp,
        Some("httpupgrade") => Transport::Httpupgrade,
        _ => Transport::Tcp,
    };

    let ws = if transport == Transport::Ws {
        Some(WsSettings {
            path: params.get("path").unwrap_or_else(|| "/".to_string()),
            host: params.get("host"),
        })
    } else {
        None
    };

    let grpc = if transport == Transport::Grpc {
        Some(GrpcSettings {
            service_name: params.get("serviceName").unwrap_or_default(),
        })
    } else {
        None
    };

    let xhttp = if transport == Transport::Xhttp {
        Some(XhttpSettings {
            path: params.get("path").unwrap_or_else(|| "/".to_string()),
            host: params.get("host"),
            mode: params.get("mode"),
        })
    } else {
        None
    };

    let httpupgrade = if transport == Transport::Httpupgrade {
        Some(HttpupgradeSettings {
            path: params.get("path").unwrap_or_else(|| "/".to_string()),
            host: params.get("host"),
        })
    } else {
        None
    };

    let security = params.get("security").unwrap_or_else(|| "none".to_string());
    let reality = if security == "reality" {
        Some(RealitySettings {
            public_key: params.get("pbk").unwrap_or_default(),
            short_id: params.get("sid").unwrap_or_default(),
        })
    } else {
        None
    };

    let tls = TlsSettings {
        enabled: security == "tls" || security == "reality",
        server_name: params.get("sni"),
        insecure: params.get("allowInsecure").as_deref() == Some("1"),
        alpn: params
            .get("alpn")
            .map(|a| a.split(',').map(String::from).collect())
            .unwrap_or_default(),
        fingerprint: params.get("fp"),
        reality,
    };

    Ok(Server {
        id: Uuid::new_v4().to_string(),
        name,
        address: host,
        port,
        protocol: Protocol::Vless,
        uuid: Some(uuid),
        password: None,
        method: None,
        flow: params.get("flow"),
        alter_id: None,
        transport,
        ws,
        grpc,
        xhttp,
        httpupgrade,
        tls,
        subscription_id: None,
        latency_ms: None,
        favorite: false,
    })
}

// ── VMess ──────────────────────────────────────────────

fn parse_vmess(link: &str) -> Result<Server> {
    // vmess://base64json
    let encoded = link.strip_prefix("vmess://").ok_or(anyhow!("Invalid vmess link"))?;

    let decoded = general_purpose::STANDARD
        .decode(encoded.trim())
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded.trim()))
        .or_else(|_| general_purpose::URL_SAFE_NO_PAD.decode(encoded.trim()))?;

    let json: serde_json::Value = serde_json::from_slice(&decoded)?;

    let host = json_str(&json, "add")?;
    let port = json["port"]
        .as_u64()
        .or_else(|| json["port"].as_str()?.parse().ok())
        .unwrap_or(443) as u16;
    let uuid = json_str(&json, "id")?;
    let name = json["ps"].as_str().unwrap_or("VMess Server").to_string();
    let aid = json["aid"]
        .as_u64()
        .or_else(|| json["aid"].as_str()?.parse().ok())
        .unwrap_or(0) as u32;

    let net = json["net"].as_str().unwrap_or("tcp");
    let transport = match net {
        "ws" => Transport::Ws,
        "grpc" => Transport::Grpc,
        "h2" | "http" => Transport::Http,
        "quic" => Transport::Quic,
        "xhttp" | "splithttp" => Transport::Xhttp,
        "httpupgrade" => Transport::Httpupgrade,
        _ => Transport::Tcp,
    };

    let ws = if transport == Transport::Ws {
        Some(WsSettings {
            path: json["path"].as_str().unwrap_or("/").to_string(),
            host: json["host"].as_str().map(String::from),
        })
    } else {
        None
    };

    let tls_val = json["tls"].as_str().unwrap_or("");
    let tls = TlsSettings {
        enabled: tls_val == "tls",
        server_name: json["sni"].as_str().map(String::from),
        insecure: false,
        alpn: vec![],
        fingerprint: json["fp"].as_str().map(String::from),
        reality: None,
    };

    Ok(Server {
        id: Uuid::new_v4().to_string(),
        name,
        address: host,
        port,
        protocol: Protocol::Vmess,
        uuid: Some(uuid),
        password: None,
        method: None,
        flow: None,
        alter_id: Some(aid),
        transport,
        ws,
        grpc: None,
        xhttp: None,
        httpupgrade: None,
        tls,
        subscription_id: None,
        latency_ms: None,
        favorite: false,
    })
}

// ── Shadowsocks ────────────────────────────────────────

fn parse_shadowsocks(link: &str) -> Result<Server> {
    // Format 1: ss://base64(method:password)@host:port#name
    // Format 2: ss://base64(method:password@host:port)#name
    let without_prefix = link.strip_prefix("ss://").ok_or(anyhow!("Invalid ss link"))?;

    let (main_part, name) = match without_prefix.split_once('#') {
        Some((m, n)) => (m, percent_decode(n)),
        None => (without_prefix, "SS Server".to_string()),
    };

    // Try format 1: base64@host:port
    if let Some((encoded, server_part)) = main_part.split_once('@') {
        let decoded = decode_b64(encoded)?;
        let (method, password) = decoded
            .split_once(':')
            .ok_or(anyhow!("Invalid ss userinfo"))?;

        let (host, port) = parse_host_port(server_part)?;

        return Ok(make_ss_server(
            name,
            host,
            port,
            method.to_string(),
            password.to_string(),
        ));
    }

    // Try format 2: everything is base64
    let decoded = decode_b64(main_part)?;
    if let Some((userinfo, server_part)) = decoded.split_once('@') {
        let (method, password) = userinfo
            .split_once(':')
            .ok_or(anyhow!("Invalid ss userinfo"))?;
        let (host, port) = parse_host_port(server_part)?;

        return Ok(make_ss_server(
            name,
            host,
            port,
            method.to_string(),
            password.to_string(),
        ));
    }

    Err(anyhow!("Could not parse ss link"))
}

fn make_ss_server(name: String, address: String, port: u16, method: String, password: String) -> Server {
    Server {
        id: Uuid::new_v4().to_string(),
        name,
        address,
        port,
        protocol: Protocol::Shadowsocks,
        uuid: None,
        password: Some(password),
        method: Some(method),
        flow: None,
        alter_id: None,
        transport: Transport::Tcp,
        ws: None,
        grpc: None,
        xhttp: None,
        httpupgrade: None,
        tls: TlsSettings::default(),
        subscription_id: None,
        latency_ms: None,
        favorite: false,
    }
}

// ── Trojan ─────────────────────────────────────────────

fn parse_trojan(link: &str) -> Result<Server> {
    // trojan://password@host:port?params#name
    let url = Url::parse(link)?;

    let password = url.username().to_string();
    let host = url.host_str().ok_or(anyhow!("No host"))?.to_string();
    let port = url.port().unwrap_or(443);
    let name = percent_decode(url.fragment().unwrap_or("Trojan Server"));

    let params = QueryParams::from_url(&url);

    let transport = match params.get("type").as_deref() {
        Some("ws") => Transport::Ws,
        Some("grpc") => Transport::Grpc,
        Some("xhttp") | Some("splithttp") => Transport::Xhttp,
        Some("httpupgrade") => Transport::Httpupgrade,
        _ => Transport::Tcp,
    };

    let ws = if transport == Transport::Ws {
        Some(WsSettings {
            path: params.get("path").unwrap_or_else(|| "/".to_string()),
            host: params.get("host"),
        })
    } else {
        None
    };

    let tls = TlsSettings {
        enabled: true,
        server_name: params.get("sni"),
        insecure: params.get("allowInsecure").as_deref() == Some("1"),
        alpn: params
            .get("alpn")
            .map(|a| a.split(',').map(String::from).collect())
            .unwrap_or_default(),
        fingerprint: params.get("fp"),
        reality: None,
    };

    Ok(Server {
        id: Uuid::new_v4().to_string(),
        name,
        address: host,
        port,
        protocol: Protocol::Trojan,
        uuid: None,
        password: Some(password),
        method: None,
        flow: None,
        alter_id: None,
        transport,
        ws,
        grpc: None,
        xhttp: None,
        httpupgrade: None,
        tls,
        subscription_id: None,
        latency_ms: None,
        favorite: false,
    })
}

// ── Hysteria2 ─────────────────────────────────────────

fn parse_hysteria2(link: &str) -> Result<Server> {
    // hy2://password@host:port?params#name
    // hysteria2://password@host:port?params#name
    let normalized = if link.starts_with("hy2://") {
        link.replacen("hy2://", "hysteria2://", 1)
    } else {
        link.to_string()
    };
    let url = Url::parse(&normalized)?;

    let password = url.username().to_string();
    let host = url.host_str().ok_or(anyhow!("No host"))?.to_string();
    let port = url.port().unwrap_or(443);
    let name = percent_decode(url.fragment().unwrap_or("Hysteria2 Server"));

    let params = QueryParams::from_url(&url);

    let tls = TlsSettings {
        enabled: true,
        server_name: params.get("sni"),
        insecure: params.get("insecure").as_deref() == Some("1"),
        alpn: params
            .get("alpn")
            .map(|a| a.split(',').map(String::from).collect())
            .unwrap_or_else(|| vec!["h3".to_string()]),
        fingerprint: params.get("fp"),
        reality: None,
    };

    Ok(Server {
        id: Uuid::new_v4().to_string(),
        name,
        address: host,
        port,
        protocol: Protocol::Hysteria2,
        uuid: None,
        password: Some(password),
        method: None,
        flow: None,
        alter_id: None,
        transport: Transport::Quic,
        ws: None,
        grpc: None,
        xhttp: None,
        httpupgrade: None,
        tls,
        subscription_id: None,
        latency_ms: None,
        favorite: false,
    })
}

// ── Helpers ────────────────────────────────────────────

struct QueryParams(Vec<(String, String)>);

impl QueryParams {
    fn from_url(url: &Url) -> Self {
        Self(url.query_pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect())
    }

    fn get(&self, key: &str) -> Option<String> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    }
}

fn json_str(json: &serde_json::Value, key: &str) -> Result<String> {
    json[key]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow!("Missing field: {}", key))
}

fn percent_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .to_string()
}

fn decode_b64(s: &str) -> Result<String> {
    let bytes = general_purpose::STANDARD
        .decode(s.trim())
        .or_else(|_| general_purpose::URL_SAFE.decode(s.trim()))
        .or_else(|_| general_purpose::URL_SAFE_NO_PAD.decode(s.trim()))?;
    Ok(String::from_utf8(bytes)?)
}

fn parse_host_port(s: &str) -> Result<(String, u16)> {
    let s = s.trim();
    if let Some(idx) = s.rfind(':') {
        let host = s[..idx].to_string();
        let port: u16 = s[idx + 1..].parse()?;
        Ok((host, port))
    } else {
        Err(anyhow!("Cannot parse host:port from '{}'", s))
    }
}

/// Convert a Server back into a shareable protocol link
pub fn server_to_link(server: &Server) -> String {
    match server.protocol {
        Protocol::Vless => {
            let uuid = server.uuid.as_deref().unwrap_or("");
            let mut params = vec![];

            // Transport
            let transport_type = match server.transport {
                Transport::Ws => "ws",
                Transport::Grpc => "grpc",
                Transport::Http => "http",
                Transport::Quic => "quic",
                Transport::Xhttp => "xhttp",
                Transport::Httpupgrade => "httpupgrade",
                Transport::Tcp => "tcp",
            };
            params.push(format!("type={}", transport_type));

            if let Some(ws) = &server.ws {
                params.push(format!("path={}", percent_encode(&ws.path)));
                if let Some(host) = &ws.host {
                    params.push(format!("host={}", host));
                }
            }
            if let Some(grpc) = &server.grpc {
                params.push(format!("serviceName={}", grpc.service_name));
            }
            if let Some(xhttp) = &server.xhttp {
                params.push(format!("path={}", percent_encode(&xhttp.path)));
                if let Some(host) = &xhttp.host {
                    params.push(format!("host={}", host));
                }
                if let Some(mode) = &xhttp.mode {
                    params.push(format!("mode={}", mode));
                }
            }
            if let Some(hu) = &server.httpupgrade {
                params.push(format!("path={}", percent_encode(&hu.path)));
                if let Some(host) = &hu.host {
                    params.push(format!("host={}", host));
                }
            }

            // TLS
            if server.tls.enabled {
                if server.tls.reality.is_some() {
                    params.push("security=reality".to_string());
                    let r = server.tls.reality.as_ref().unwrap();
                    params.push(format!("pbk={}", r.public_key));
                    params.push(format!("sid={}", r.short_id));
                } else {
                    params.push("security=tls".to_string());
                }
                if let Some(sni) = &server.tls.server_name {
                    params.push(format!("sni={}", sni));
                }
                if let Some(fp) = &server.tls.fingerprint {
                    params.push(format!("fp={}", fp));
                }
                if !server.tls.alpn.is_empty() {
                    params.push(format!("alpn={}", server.tls.alpn.join(",")));
                }
            }

            if let Some(flow) = &server.flow {
                if !flow.is_empty() {
                    params.push(format!("flow={}", flow));
                }
            }

            let name = percent_encode(&server.name);
            format!("vless://{}@{}:{}?{}#{}", uuid, server.address, server.port, params.join("&"), name)
        }
        Protocol::Vmess => {
            let obj = serde_json::json!({
                "v": "2",
                "ps": server.name,
                "add": server.address,
                "port": server.port,
                "id": server.uuid.as_deref().unwrap_or(""),
                "aid": server.alter_id.unwrap_or(0),
                "net": match server.transport {
                    Transport::Ws => "ws",
                    Transport::Grpc => "grpc",
                    Transport::Http => "h2",
                    Transport::Xhttp => "xhttp",
                    Transport::Httpupgrade => "httpupgrade",
                    _ => "tcp",
                },
                "type": "none",
                "host": server.ws.as_ref().and_then(|w| w.host.as_deref()).unwrap_or(""),
                "path": server.ws.as_ref().map(|w| w.path.as_str()).unwrap_or(""),
                "tls": if server.tls.enabled { "tls" } else { "" },
                "sni": server.tls.server_name.as_deref().unwrap_or(""),
            });
            format!("vmess://{}", general_purpose::STANDARD.encode(obj.to_string()))
        }
        Protocol::Shadowsocks => {
            let method = server.method.as_deref().unwrap_or("aes-256-gcm");
            let password = server.password.as_deref().unwrap_or("");
            let user_info = general_purpose::STANDARD.encode(format!("{}:{}", method, password));
            let name = percent_encode(&server.name);
            format!("ss://{}@{}:{}#{}", user_info, server.address, server.port, name)
        }
        Protocol::Trojan => {
            let password = server.password.as_deref().unwrap_or("");
            let mut params = vec![];
            if let Some(sni) = &server.tls.server_name {
                params.push(format!("sni={}", sni));
            }
            let name = percent_encode(&server.name);
            let query = if params.is_empty() { String::new() } else { format!("?{}", params.join("&")) };
            format!("trojan://{}@{}:{}{}#{}", password, server.address, server.port, query, name)
        }
        Protocol::Hysteria2 => {
            let password = server.password.as_deref().unwrap_or("");
            let name = percent_encode(&server.name);
            format!("hy2://{}@{}:{}#{}", password, server.address, server.port, name)
        }
        Protocol::Tuic => {
            let uuid = server.uuid.as_deref().unwrap_or("");
            let password = server.password.as_deref().unwrap_or("");
            let name = percent_encode(&server.name);
            format!("tuic://{}:{}@{}:{}#{}", uuid, password, server.address, server.port, name)
        }
    }
}

fn percent_encode(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vless() {
        let link = "vless://uuid-here@example.com:443?security=tls&type=ws&path=%2Fws&sni=example.com#Test%20Server";
        let server = parse_link(link).unwrap();
        assert_eq!(server.protocol, Protocol::Vless);
        assert_eq!(server.address, "example.com");
        assert_eq!(server.port, 443);
        assert_eq!(server.name, "Test Server");
        assert!(server.tls.enabled);
    }

    #[test]
    fn test_parse_trojan() {
        let link = "trojan://password123@example.com:443?sni=example.com#Trojan";
        let server = parse_link(link).unwrap();
        assert_eq!(server.protocol, Protocol::Trojan);
        assert_eq!(server.password, Some("password123".to_string()));
    }
}
