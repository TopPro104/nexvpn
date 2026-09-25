use anyhow::Result;
use serde_json::{json, Value};

use crate::proxy::models::*;
use crate::proxy::routing::{DnsKind, DnsSpec, EffectiveRouting, Matcher, RouteAction};

/// Generate a minimal Xray-core config for a single server
pub fn generate_config(server: &Server, socks_port: u16, http_port: u16, routing: &EffectiveRouting, auth: (&str, &str), api_port: u16) -> Result<Value> {
    let outbound = build_outbound(server)?;
    let (auth_user, auth_pass) = auth;

    let config = json!({
        "log": {
            "loglevel": "warning"
        },
        "dns": build_xray_dns(routing),
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
                "settings": if cfg!(target_os = "android") {
                    json!({
                        "auth": "password",
                        "accounts": [{ "user": auth_user, "pass": auth_pass }],
                        "udp": true
                    })
                } else {
                    json!({ "udp": true })
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
                "settings": if cfg!(target_os = "android") {
                    json!({
                        "accounts": [{ "user": auth_user, "pass": auth_pass }]
                    })
                } else {
                    json!({})
                },
                "sniffing": {
                    "enabled": true,
                    "destOverride": ["http", "tls"]
                }
            },
            {
                "tag": "api-in",
                "port": api_port,
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
            "domainStrategy": if routing.resolve_ips { "IPIfNonMatch" } else { "AsIs" },
            "rules": build_xray_routing_rules(routing)
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
        Protocol::Hysteria2 => {
            // Xray-core v26.1.23+ native Hysteria2 — protocol "hysteria" version 2;
            // transport "hysteria" carries auth. See xtls.github.io/en/config/outbounds/hysteria.html
            out["protocol"] = json!("hysteria");
            out["settings"] = json!({
                "version": 2,
                "address": server.address,
                "port": server.port
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

    // Hysteria2 forces its own transport regardless of server.transport.
    if matches!(server.protocol, Protocol::Hysteria2) {
        stream["network"] = json!("hysteria");
        stream["hysteriaSettings"] = json!({
            "version": 2,
            "auth": server.password.as_deref().unwrap_or("")
        });
    }

    // Transport
    if !matches!(server.protocol, Protocol::Hysteria2) {
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
        Transport::Xhttp => {
            stream["network"] = json!("xhttp");
            let xhttp = server.xhttp.as_ref();
            let mut settings = json!({
                "path": xhttp.map(|x| x.path.as_str()).unwrap_or("/")
            });
            if let Some(host) = xhttp.and_then(|x| x.host.as_deref()) {
                settings["host"] = json!(host);
            }
            if let Some(mode) = xhttp.and_then(|x| x.mode.as_deref()) {
                settings["mode"] = json!(mode);
            }
            stream["xhttpSettings"] = settings;
        }
        Transport::Httpupgrade => {
            stream["network"] = json!("httpupgrade");
            let hu = server.httpupgrade.as_ref();
            let mut settings = json!({
                "path": hu.map(|h| h.path.as_str()).unwrap_or("/")
            });
            if let Some(host) = hu.and_then(|h| h.host.as_deref()) {
                settings["host"] = json!(host);
            }
            stream["httpupgradeSettings"] = settings;
        }
        _ => {}
    }
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

/// Xray matcher syntax; geo codes missing from the .dat files are dropped because Xray
/// refuses to start on an unknown code.
fn xray_entry(m: &Matcher, routing: &EffectiveRouting) -> Option<String> {
    if !routing.geo_usable_xray(m) {
        return None;
    }
    Some(match m {
        Matcher::Suffix(d) => format!("domain:{}", d),
        Matcher::Full(d) => format!("full:{}", d),
        Matcher::Keyword(d) => format!("keyword:{}", d),
        Matcher::Regex(d) => format!("regexp:{}", d),
        Matcher::GeoSite(c) => format!("geosite:{}", c),
        Matcher::GeoIp(c) => format!("geoip:{}", c),
        Matcher::Cidr(c) => c.clone(),
    })
}

fn build_xray_routing_rules(routing: &EffectiveRouting) -> Value {
    let mut rules = vec![
        json!({
            "inboundTag": ["api-in"],
            "outboundTag": "api",
            "type": "field"
        }),
    ];

    // Pin the profile's resolvers: remote through the proxy, domestic direct
    if let Some(dns) = &routing.dns {
        for (spec, tag) in [(&dns.remote, "proxy"), (&dns.domestic, "direct")] {
            if spec.ip.parse::<std::net::IpAddr>().is_ok() {
                rules.push(json!({ "type": "field", "ip": [spec.ip], "outboundTag": tag }));
            }
        }
    }

    for group in &routing.groups {
        let tag = match group.action {
            RouteAction::Direct => "direct",
            RouteAction::Block => "block",
            RouteAction::Proxy => "proxy",
        };
        // domain and ip in one Xray rule would be ANDed — keep them separate
        let domains: Vec<String> = group.domains.iter().filter_map(|m| xray_entry(m, routing)).collect();
        if !domains.is_empty() {
            rules.push(json!({ "type": "field", "domain": domains, "outboundTag": tag }));
        }
        let ips: Vec<String> = group.ips.iter().filter_map(|m| xray_entry(m, routing)).collect();
        if !ips.is_empty() {
            rules.push(json!({ "type": "field", "ip": ips, "outboundTag": tag }));
        }
    }

    // Unmatched traffic goes to the first outbound (proxy) unless routed direct
    if !routing.final_proxy {
        rules.push(json!({
            "type": "field",
            "network": "tcp,udp",
            "outboundTag": "direct"
        }));
    }

    json!(rules)
}

fn build_xray_dns(routing: &EffectiveRouting) -> Value {
    let Some(dns) = &routing.dns else {
        return json!({
            "servers": [
                "https+local://1.1.1.1/dns-query",
                "localhost"
            ]
        });
    };

    let mut hosts = serde_json::Map::new();
    for (host, ip) in &dns.hosts {
        hosts.insert(host.clone(), json!(ip));
    }
    // "+local" DoH skips routing (direct); plain DoH/UDP go through the rules pinned above
    let address = |spec: &DnsSpec, local: bool, hosts: &mut serde_json::Map<String, Value>| -> String {
        match (&spec.kind, spec.doh_parts()) {
            (DnsKind::DoH, Some((host, _, _))) => {
                if host.parse::<std::net::IpAddr>().is_err() && !spec.ip.is_empty() {
                    hosts.entry(host.to_lowercase()).or_insert_with(|| json!(spec.ip));
                }
                if local { spec.url.replacen("https://", "https+local://", 1) } else { spec.url.clone() }
            }
            _ => spec.ip.clone(),
        }
    };
    let remote = address(&dns.remote, false, &mut hosts);
    let domestic = address(&dns.domestic, true, &mut hosts);

    let direct_domains: Vec<String> = routing
        .groups
        .iter()
        .filter(|g| g.action == RouteAction::Direct)
        .flat_map(|g| g.domains.iter())
        .filter_map(|m| xray_entry(m, routing))
        .collect();

    let mut servers = vec![json!(remote)];
    let mut domestic_server = json!({ "address": domestic, "skipFallback": true });
    if !direct_domains.is_empty() {
        domestic_server["domains"] = json!(direct_domains);
    }
    servers.push(domestic_server);
    if !routing.final_proxy {
        // Direct by default: unmatched names resolve directly, proxied ones remotely
        servers.reverse();
        servers[0] = json!(domestic);
        let proxy_domains: Vec<String> = routing
            .groups
            .iter()
            .filter(|g| g.action == RouteAction::Proxy)
            .flat_map(|g| g.domains.iter())
            .filter_map(|m| xray_entry(m, routing))
            .collect();
        let mut remote_server = json!({ "address": remote, "skipFallback": true });
        if !proxy_domains.is_empty() {
            remote_server["domains"] = json!(proxy_domains);
        }
        servers[1] = remote_server;
    }

    json!({ "hosts": hosts, "servers": servers })
}
