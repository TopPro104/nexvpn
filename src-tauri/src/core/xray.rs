use anyhow::Result;
use serde_json::{json, Value};

use crate::proxy::models::*;

/// Generate a minimal Xray-core config for a single server
pub fn generate_config(server: &Server, socks_port: u16, http_port: u16) -> Result<Value> {
    let outbound = build_outbound(server)?;

    let config = json!({
        "log": {
            "loglevel": "warning"
        },
        "dns": {
            "servers": [
                "https+local://1.1.1.1/dns-query",
                "localhost"
            ]
        },
        "stats": {},
        "api": {
            "tag": "api",
            "services": ["StatsService"]
        },
        "policy": {
            "system": {
                "statsInboundUplink": true,
                "statsInboundDownlink": true,
                "statsOutboundUplink": true,
                "statsOutboundDownlink": true
            }
        },
        "inbounds": [
            {
                "tag": "socks-in",
                "port": socks_port,
                "listen": "127.0.0.1",
                "protocol": "socks",
                "settings": {
                    "udp": true
                },
                "sniffing": {
                    "enabled": true,
                    "destOverride": ["http", "tls"]
                }
            },
            {
                "tag": "http-in",
                "port": http_port,
                "listen": "127.0.0.1",
                "protocol": "http",
                "sniffing": {
                    "enabled": true,
                    "destOverride": ["http", "tls"]
                }
            },
            {
                "tag": "api-in",
                "port": 10813,
                "listen": "127.0.0.1",
                "protocol": "dokodemo-door",
                "settings": {
                    "address": "127.0.0.1"
                }
            }
        ],
        "outbounds": [
            outbound,
            {
                "tag": "direct",
                "protocol": "freedom"
            },
            {
                "tag": "block",
                "protocol": "blackhole"
            }
        ],
        "routing": {
            "domainStrategy": "AsIs",
            "rules": [
                {
                    "inboundTag": ["api-in"],
                    "outboundTag": "api",
                    "type": "field"
                }
            ]
        }
    });

    Ok(config)
}

fn build_outbound(server: &Server) -> Result<Value> {
    let mut out = json!({"tag": "proxy"});

    match server.protocol {
        Protocol::Vless => {
            out["protocol"] = json!("vless");
            let mut user = json!({
                "id": server.uuid.as_deref().unwrap_or(""),
                "encryption": "none"
            });
            if let Some(flow) = &server.flow {
                if !flow.is_empty() {
                    user["flow"] = json!(flow);
                }
            }
            out["settings"] = json!({
                "vnext": [{
                    "address": server.address,
                    "port": server.port,
                    "users": [user]
                }]
            });
        }
        Protocol::Vmess => {
            out["protocol"] = json!("vmess");
            out["settings"] = json!({
                "vnext": [{
                    "address": server.address,
                    "port": server.port,
                    "users": [{
                        "id": server.uuid.as_deref().unwrap_or(""),
                        "alterId": server.alter_id.unwrap_or(0),
                        "security": "auto"
                    }]
                }]
            });
        }
        Protocol::Shadowsocks => {
            out["protocol"] = json!("shadowsocks");
            out["settings"] = json!({
                "servers": [{
                    "address": server.address,
                    "port": server.port,
                    "method": server.method.as_deref().unwrap_or("aes-256-gcm"),
                    "password": server.password.as_deref().unwrap_or("")
                }]
            });
        }
        Protocol::Trojan => {
            out["protocol"] = json!("trojan");
            out["settings"] = json!({
                "servers": [{
                    "address": server.address,
                    "port": server.port,
                    "password": server.password.as_deref().unwrap_or("")
                }]
            });
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Protocol {:?} not supported by Xray-core",
                server.protocol
            ));
        }
    }

    // Stream settings
    let mut stream = json!({});

    // Transport
    match server.transport {
        Transport::Ws => {
            stream["network"] = json!("ws");
            let ws = server.ws.as_ref();
            let mut ws_settings = json!({
                "path": ws.map(|w| w.path.as_str()).unwrap_or("/")
            });
            if let Some(host) = ws.and_then(|w| w.host.as_deref()) {
                ws_settings["headers"] = json!({"Host": host});
            }
            stream["wsSettings"] = ws_settings;
        }
        Transport::Grpc => {
            stream["network"] = json!("grpc");
            let grpc = server.grpc.as_ref();
            stream["grpcSettings"] = json!({
                "serviceName": grpc.map(|g| g.service_name.as_str()).unwrap_or("")
            });
        }
        Transport::Http => {
            stream["network"] = json!("h2");
        }
        Transport::Tcp => {
            stream["network"] = json!("tcp");
        }
        _ => {}
    }

    // TLS
    if server.tls.enabled {
        if server.tls.reality.is_some() {
            stream["security"] = json!("reality");
            let reality = server.tls.reality.as_ref().unwrap();
            let mut rs = json!({
                "publicKey": reality.public_key,
                "shortId": reality.short_id,
                "fingerprint": server.tls.fingerprint.as_deref().unwrap_or("chrome")
            });
            if let Some(sni) = &server.tls.server_name {
                rs["serverName"] = json!(sni);
            }
            stream["realitySettings"] = rs;
        } else {
            stream["security"] = json!("tls");
            let mut tls = json!({});
            if let Some(sni) = &server.tls.server_name {
                tls["serverName"] = json!(sni);
            }
            if server.tls.insecure {
                tls["allowInsecure"] = json!(true);
            }
            if !server.tls.alpn.is_empty() {
                tls["alpn"] = json!(server.tls.alpn);
            }
            if let Some(fp) = &server.tls.fingerprint {
                tls["fingerprint"] = json!(fp);
            }
            stream["tlsSettings"] = tls;
        }
    } else {
        stream["security"] = json!("none");
    }

    out["streamSettings"] = stream;

    Ok(out)
}
