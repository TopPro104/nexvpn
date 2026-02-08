use anyhow::Result;
use serde_json::{json, Value};

use crate::proxy::models::*;

/// Generate a sing-box config for connecting to a single server
pub fn generate_config(server: &Server, socks_port: u16, http_port: u16, tun_mode: bool) -> Result<Value> {
    let outbound = build_outbound(server)?;

    // sing-box 1.12+ config format:
    // - DNS servers use "type" field
    // - No block/dns outbounds — use rule actions (sniff, hijack-dns)
    // - default_domain_resolver replaces outbound DNS rules
    let mut inbounds = vec![
        json!({
            "type": "socks",
            "tag": "socks-in",
            "listen": "127.0.0.1",
            "listen_port": socks_port
        }),
        json!({
            "type": "http",
            "tag": "http-in",
            "listen": "127.0.0.1",
            "listen_port": http_port
        }),
    ];

    if tun_mode {
        inbounds.push(json!({
            "type": "tun",
            "tag": "tun-in",
            "address": [
                "172.19.0.1/30",
                "fdfe:dcba:9876::1/126"
            ],
            "auto_route": true,
            "strict_route": true,
            "stack": "mixed"
        }));
    }

    let config = json!({
        "log": {
            "level": "warn",
            "timestamp": true
        },
        "dns": {
            "servers": [
                {
                    "tag": "dns-local",
                    "type": "local"
                },
                {
                    "tag": "dns-remote",
                    "type": "udp",
                    "server": "8.8.8.8"
                }
            ],
            "final": "dns-remote"
        },
        "inbounds": inbounds,
        "outbounds": [
            outbound,
            {
                "type": "direct",
                "tag": "direct"
            }
        ],
        "route": {
            "rules": [
                {
                    "action": "sniff"
                },
                {
                    "protocol": "dns",
                    "action": "hijack-dns"
                }
            ],
            "final": "proxy",
            "auto_detect_interface": true,
            "default_domain_resolver": {
                "server": "dns-local"
            }
        },
        "experimental": {
            "clash_api": {
                "external_controller": "127.0.0.1:9090"
            }
        }
    });

    Ok(config)
}

fn build_outbound(server: &Server) -> Result<Value> {
    let mut out = json!({
        "tag": "proxy",
        "server": server.address,
        "server_port": server.port,
    });

    match server.protocol {
        Protocol::Vless => {
            out["type"] = json!("vless");
            out["uuid"] = json!(server.uuid.as_deref().unwrap_or(""));
            if let Some(flow) = &server.flow {
                if !flow.is_empty() {
                    out["flow"] = json!(flow);
                }
            }
        }
        Protocol::Vmess => {
            out["type"] = json!("vmess");
            out["uuid"] = json!(server.uuid.as_deref().unwrap_or(""));
            out["alter_id"] = json!(server.alter_id.unwrap_or(0));
            out["security"] = json!("auto");
        }
        Protocol::Shadowsocks => {
            out["type"] = json!("shadowsocks");
            out["method"] = json!(server.method.as_deref().unwrap_or("aes-256-gcm"));
            out["password"] = json!(server.password.as_deref().unwrap_or(""));
        }
        Protocol::Trojan => {
            out["type"] = json!("trojan");
            out["password"] = json!(server.password.as_deref().unwrap_or(""));
        }
        Protocol::Hysteria2 => {
            out["type"] = json!("hysteria2");
            out["password"] = json!(server.password.as_deref().unwrap_or(""));
        }
        Protocol::Tuic => {
            out["type"] = json!("tuic");
            out["uuid"] = json!(server.uuid.as_deref().unwrap_or(""));
            out["password"] = json!(server.password.as_deref().unwrap_or(""));
        }
    }

    // Transport
    match server.transport {
        Transport::Ws => {
            let ws = server.ws.as_ref();
            let mut transport = json!({
                "type": "ws",
                "path": ws.map(|w| w.path.as_str()).unwrap_or("/")
            });
            if let Some(host) = ws.and_then(|w| w.host.as_deref()) {
                transport["headers"] = json!({"Host": host});
            }
            out["transport"] = transport;
        }
        Transport::Grpc => {
            let grpc = server.grpc.as_ref();
            out["transport"] = json!({
                "type": "grpc",
                "service_name": grpc.map(|g| g.service_name.as_str()).unwrap_or("")
            });
        }
        Transport::Http => {
            out["transport"] = json!({"type": "http"});
        }
        _ => {}
    }

    // TLS
    if server.tls.enabled {
        let mut tls = json!({
            "enabled": true
        });
        if let Some(sni) = &server.tls.server_name {
            tls["server_name"] = json!(sni);
        }
        if server.tls.insecure {
            tls["insecure"] = json!(true);
        }
        if !server.tls.alpn.is_empty() {
            tls["alpn"] = json!(server.tls.alpn);
        }
        if let Some(fp) = &server.tls.fingerprint {
            tls["utls"] = json!({"enabled": true, "fingerprint": fp});
        }
        if let Some(reality) = &server.tls.reality {
            tls["reality"] = json!({
                "enabled": true,
                "public_key": reality.public_key,
                "short_id": reality.short_id
            });
        }
        out["tls"] = tls;
    }

    Ok(out)
}
