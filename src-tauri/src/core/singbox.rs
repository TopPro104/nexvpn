use anyhow::Result;
use serde_json::{json, Value};

use crate::proxy::models::*;

/// Generate a sing-box config for connecting to a single server
pub fn generate_config(server: &Server, socks_port: u16, http_port: u16, tun_mode: bool, routing_rules: &[RoutingRule], default_route: &str) -> Result<Value> {
    let outbound = build_outbound(server)?;

    // On Android, TUN requires VpnService — force disable
    let tun_mode = if cfg!(target_os = "android") { false } else { tun_mode };

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
            "strict_route": false,
            "stack": "mixed",
            "endpoint_independent_nat": true
        }));
    }

    // DNS config: TUN mode needs DoH + proper resolver chain to avoid loops
    // sing-box 1.12+ new DNS format: use type/server instead of address
    let dns = if tun_mode {
        json!({
            "servers": [
                {
                    "tag": "dns-remote",
                    "type": "https",
                    "server": "dns.google",
                    "server_port": 443,
                    "domain_resolver": "dns-direct",
                    "detour": "proxy"
                },
                {
                    "tag": "dns-direct",
                    "type": "udp",
                    "server": "8.8.8.8",
                    "server_port": 53
                }
            ],
            "rules": [
                { "query_type": [28, 32, 33], "action": "reject" },
                { "domain_suffix": [".lan"], "action": "reject" }
            ],
            "final": "dns-remote",
            "independent_cache": true
        })
    } else {
        json!({
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
        })
    };

    // Route rules (sniff + DNS hijack, same approach as NekoRay)
    let mut route_rules: Vec<Value> = vec![
        json!({ "action": "sniff" }),
        json!({ "protocol": "dns", "action": "hijack-dns" }),
    ];

    if tun_mode {
        // Block multicast, NetBIOS, mDNS — they shouldn't go through proxy
        route_rules.push(json!({
            "network": "udp",
            "port": [135, 137, 138, 139, 5353],
            "action": "reject"
        }));
        route_rules.push(json!({
            "ip_cidr": ["224.0.0.0/3", "ff00::/8"],
            "action": "reject"
        }));
        route_rules.push(json!({
            "source_ip_cidr": ["224.0.0.0/3", "ff00::/8"],
            "action": "reject"
        }));
    }

    // User-defined routing rules
    for rule in routing_rules.iter().filter(|r| r.enabled) {
        let domain = &rule.domain;
        match rule.action {
            RuleAction::Direct => {
                route_rules.push(json!({ "domain_suffix": [domain], "outbound": "direct" }));
            }
            RuleAction::Block => {
                route_rules.push(json!({ "domain_suffix": [domain], "action": "reject" }));
            }
            RuleAction::Proxy => {
                route_rules.push(json!({ "domain_suffix": [domain], "outbound": "proxy" }));
            }
        }
    }

    let final_route = if default_route == "direct" { "direct" } else { "proxy" };

    let config = json!({
        "log": {
            "level": if cfg!(target_os = "android") { "info" } else { "warn" },
            "timestamp": true
        },
        "dns": dns,
        "inbounds": inbounds,
        "outbounds": [
            outbound,
            {
                "type": "direct",
                "tag": "direct"
            }
        ],
        "route": {
            "rules": route_rules,
            "final": final_route,
            "auto_detect_interface": !cfg!(target_os = "android"),
            "default_domain_resolver": {
                "server": if cfg!(target_os = "android") { "dns-remote" } else if tun_mode { "dns-direct" } else { "dns-local" }
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
        "udp_fragment": true,
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
