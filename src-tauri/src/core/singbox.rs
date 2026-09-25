use anyhow::Result;
use serde_json::{json, Value};

use crate::proxy::models::*;
use crate::proxy::routing::{DnsKind, DnsSpec, EffectiveRouting, Matcher, RouteAction};

/// Generate a sing-box config for connecting to a single server
pub fn generate_config(server: &Server, socks_port: u16, http_port: u16, tun_mode: bool, routing: &EffectiveRouting, auth: (&str, &str), clash_secret: &str, clash_api_port: u16) -> Result<Value> {
    let outbound = build_outbound(server)?;
    let (auth_user, auth_pass) = auth;

    // On Android, TUN requires VpnService — force disable
    let tun_mode = if cfg!(target_os = "android") { false } else { tun_mode };

    // Android: auth protects local proxy from other apps scanning ports
    // Desktop: no auth needed, system proxy doesn't support credentials
    let use_auth = cfg!(target_os = "android");

    let mut socks_inbound = json!({
        "type": "socks",
        "tag": "socks-in",
        "listen": "127.0.0.1",
        "listen_port": socks_port
    });
    let mut http_inbound = json!({
        "type": "http",
        "tag": "http-in",
        "listen": "127.0.0.1",
        "listen_port": http_port
    });
    if use_auth {
        socks_inbound["users"] = json!([{ "username": auth_user, "password": auth_pass }]);
        http_inbound["users"] = json!([{ "username": auth_user, "password": auth_pass }]);
    }

    let mut inbounds = vec![socks_inbound, http_inbound];

    if tun_mode {
        inbounds.push(json!({
            "type": "tun",
            "tag": "tun-in",
            "address": [
                "172.19.0.1/28",
                "fdfe:dcba:9876::1/124"
            ],
            "mtu": 9000,
            "auto_route": true,
            "strict_route": false,
            "stack": "mixed",
            "udp_timeout": "5m",
            "endpoint_independent_nat": true
        }));
    }

    // DNS. Without a profile: TUN needs DoH through the proxy + a direct resolver for
    // bootstrapping; proxy mode only resolves for the direct outbound.
    // With a profile: its remote resolver goes through the proxy, the domestic one direct.
    // sing-box 1.12+ DNS format: type/server instead of address.
    let (dns_servers, remote_tag, direct_tag) = if let Some(pdns) = &routing.dns {
        let mut hosts = pdns.hosts.clone();
        let mut servers = vec![
            dns_server(&pdns.remote, "dns-remote", Some("proxy"), &mut hosts),
            dns_server(&pdns.domestic, "dns-domestic", None, &mut hosts),
        ];
        if !hosts.is_empty() {
            servers.push(json!({ "tag": "dns-hosts", "type": "hosts", "predefined": hosts }));
        }
        (servers, "dns-remote", "dns-domestic")
    } else if tun_mode {
        (vec![
            json!({
                "tag": "dns-remote",
                "type": "https",
                "server": "dns.google",
                "server_port": 443,
                "domain_resolver": "dns-direct",
                "detour": "proxy"
            }),
            json!({
                "tag": "dns-direct",
                "type": "udp",
                "server": "8.8.8.8",
                "server_port": 53
            }),
        ], "dns-remote", "dns-direct")
    } else {
        (vec![
            json!({ "tag": "dns-local", "type": "local" }),
            json!({ "tag": "dns-remote", "type": "udp", "server": "8.8.8.8" }),
        ], "dns-remote", if cfg!(target_os = "android") { "dns-remote" } else { "dns-local" })
    };

    let mut dns_rules: Vec<Value> = Vec::new();
    if let Some(pdns) = &routing.dns {
        if !pdns.hosts.is_empty() {
            dns_rules.push(json!({ "domain": pdns.hosts.keys().collect::<Vec<_>>(), "server": "dns-hosts" }));
        }
    }
    if tun_mode {
        dns_rules.push(json!({ "query_type": [28, 32, 33], "action": "reject" }));
        dns_rules.push(json!({ "domain_suffix": [".lan"], "action": "reject" }));
    }
    for group in &routing.groups {
        for mut item in domain_items(&group.domains, routing) {
            match group.action {
                RouteAction::Block => item["action"] = json!("reject"),
                RouteAction::Proxy => item["server"] = json!(remote_tag),
                RouteAction::Direct => item["server"] = json!(direct_tag),
            }
            dns_rules.push(item);
        }
    }

    // No independent_cache: since sing-box 1.14 the cache is always keyed per server
    let dns = json!({
        "servers": dns_servers,
        "rules": dns_rules,
        "final": if routing.final_proxy { remote_tag } else { direct_tag }
    });

    // Route rules (sniff + DNS hijack, same approach as NekoRay)
    let mut route_rules: Vec<Value> = vec![
        json!({ "action": "sniff", "timeout": "300ms" }),
        json!({ "protocol": "dns", "action": "hijack-dns" }),
    ];

    if tun_mode {
        // Block QUIC — forces YouTube, Discord, Roblox etc. to fall back to TCP.
        // sing-box cannot properly sniff QUIC with fragmented ClientHello (#1724),
        // and UDP:443 packets on Windows can bypass TUN routing (#2655).
        route_rules.push(json!({
            "protocol": "quic",
            "action": "reject"
        }));

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

    // Custom rules, private ranges, then the profile lists (see EffectiveRouting::build)
    let mut ip_rules: Vec<Value> = Vec::new();
    for group in &routing.groups {
        let domains = domain_items(&group.domains, routing);
        let ips = ip_items(&group.ips, routing);
        for mut item in domains.into_iter().chain(ips.iter().cloned()) {
            set_route_action(&mut item, group.action);
            route_rules.push(item);
        }
        for mut item in ips {
            set_route_action(&mut item, group.action);
            ip_rules.push(item);
        }
    }
    // IPIfNonMatch: a domain that matched nothing is resolved and checked against the IP
    // rules again (direct traffic keeps the domain for dialing; resolve only fills addresses).
    if routing.resolve_ips && !ip_rules.is_empty() {
        route_rules.push(json!({ "action": "resolve" }));
        route_rules.extend(ip_rules);
    }

    let rule_sets: Vec<Value> = routing
        .geo
        .rule_sets
        .iter()
        .map(|(_, tag, path)| json!({ "type": "local", "tag": tag, "format": "source", "path": path }))
        .collect();

    let final_route = if routing.final_proxy { "proxy" } else { "direct" };

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
            "rule_set": rule_sets,
            "final": final_route,
            "auto_detect_interface": !cfg!(target_os = "android"),
            "default_domain_resolver": { "server": direct_tag }
        },
        "experimental": {
            "clash_api": {
                "external_controller": format!("127.0.0.1:{}", clash_api_port),
                "secret": clash_secret
            }
        }
    });

    Ok(config)
}

/// Generate a minimal sing-box bridge config: TUN inbound → SOCKS outbound to Xray.
/// Used when user selects Xray core + TUN mode on desktop (Xray has no native TUN).
/// Pattern: v2rayN / nekoray — Xray handles the protocol, sing-box handles TUN.
pub fn generate_bridge_config(xray_socks_port: u16, server_address: &str, xray_process: &str, routing: &EffectiveRouting) -> Value {
    // Is server.address an IP literal or a hostname?
    let server_is_ip = server_address.parse::<std::net::IpAddr>().is_ok();
    let bypass_domain: Vec<String> = if !server_is_ip { vec![server_address.to_string()] } else { vec![] };
    let mut bypass_ip: Vec<String> = Vec::new();
    if server_is_ip {
        let is_v6 = server_address.parse::<std::net::Ipv6Addr>().is_ok();
        bypass_ip.push(format!("{}/{}", server_address, if is_v6 { 128 } else { 32 }));
    } else {
        // Pre-resolve the hostname and add its IPs to bypass_ip. For QUIC/Hysteria2
        // traffic sing-box can't always sniff SNI, so a domain-only rule misses the
        // packets and they loop back through TUN → Xray → TUN forever.
        use std::net::ToSocketAddrs;
        if let Ok(iter) = (server_address, 443u16).to_socket_addrs() {
            for addr in iter {
                let ip = addr.ip();
                let cidr = if ip.is_ipv6() { 128 } else { 32 };
                bypass_ip.push(format!("{}/{}", ip, cidr));
            }
        }
    }

    // macOS expects utun{N}; Windows/Linux use a free name.
    // v2rayN picks a random utun index to avoid conflicts with other VPN apps.
    let interface_name: String = if cfg!(target_os = "macos") {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        format!("utun{}", (seed % 90) + 10)
    } else {
        "singbox_tun".to_string()
    };

    // Without bypassing the VPN server address, Xray's outbound connection to the real
    // server hits the TUN → loops back into Xray forever. v2rayN calls this ProtectDomainList.
    // Xray's own sockets (its direct outbound, DNS lookups, the connection to the server)
    // must bypass the TUN too, or "direct" traffic loops TUN → Xray → TUN forever.
    let mut dns_rules: Vec<Value> = vec![
        json!({ "process_name": [xray_process], "server": "local" }),
    ];
    if !bypass_domain.is_empty() {
        dns_rules.push(json!({ "domain": bypass_domain.clone(), "server": "local" }));
    }
    // Domains the routing sends direct should get answers from a direct resolver
    for group in routing.groups.iter().filter(|g| g.action == RouteAction::Direct) {
        for mut item in domain_items(&group.domains, routing) {
            item["server"] = json!("local");
            dns_rules.push(item);
        }
    }
    let rule_sets: Vec<Value> = routing
        .geo
        .rule_sets
        .iter()
        .filter(|(key, _, _)| key.starts_with("geosite:"))
        .map(|(_, tag, path)| json!({ "type": "local", "tag": tag, "format": "source", "path": path }))
        .collect();
    let local_dns = routing
        .dns
        .as_ref()
        .map(|d| d.domestic.ip.clone())
        .filter(|ip| ip.parse::<std::net::IpAddr>().is_ok())
        .unwrap_or_else(|| "223.5.5.5".to_string());

    let mut route_rules: Vec<Value> = vec![
        json!({ "action": "sniff" }),
        json!({ "protocol": "dns", "action": "hijack-dns" }),
        json!({ "process_name": [xray_process], "outbound": "direct" }),
    ];
    if !bypass_domain.is_empty() {
        route_rules.push(json!({ "domain": bypass_domain.clone(), "outbound": "direct" }));
    }
    if !bypass_ip.is_empty() {
        route_rules.push(json!({ "ip_cidr": bypass_ip.clone(), "outbound": "direct" }));
    }
    route_rules.push(json!({ "network": "udp", "port": [135, 137, 138, 139, 5353], "action": "reject" }));
    route_rules.push(json!({ "ip_cidr": ["224.0.0.0/3", "ff00::/8"], "action": "reject" }));

    // Matches v2rayN's tun_singbox_dns / tun_singbox_inbound / tun_singbox_rules templates.
    // "proxy" tag is renamed to "to-xray" — it's the SOCKS outbound pointing at Xray.
    json!({
        "log": { "level": "error", "timestamp": true },
        "dns": {
            "servers": [
                { "tag": "remote", "type": "tcp", "server": "8.8.8.8", "detour": "to-xray" },
                { "tag": "local", "type": "udp", "server": local_dns }
            ],
            "rules": dns_rules,
            "final": "remote"
        },
        "inbounds": [{
            "type": "tun",
            "tag": "tun-in",
            "interface_name": interface_name,
            "address": ["172.18.0.1/30", "fdfe:dcba:9876::1/126"],
            "mtu": 9000,
            "auto_route": true,
            "strict_route": false,
            // Sniffing is the route rule above; the inbound "sniff" field was removed in 1.13
            "stack": "system"
        }],
        "outbounds": [
            {
                "type": "socks",
                "tag": "to-xray",
                "server": "127.0.0.1",
                "server_port": xray_socks_port,
                "version": "5"
            },
            { "type": "direct", "tag": "direct" }
        ],
        "route": {
            "rules": route_rules,
            "rule_set": rule_sets,
            "final": "to-xray",
            "auto_detect_interface": true,
            "default_domain_resolver": { "server": "local" }
        }
    })
}

/// A profile DNS server. DoH hostnames are pinned to the profile's IP through the hosts
/// server, so resolving the resolver never depends on another resolver.
fn dns_server(spec: &DnsSpec, tag: &str, detour: Option<&str>, hosts: &mut std::collections::BTreeMap<String, String>) -> Value {
    let mut v = match (&spec.kind, spec.doh_parts()) {
        (DnsKind::DoH, Some((host, port, path))) => {
            let mut v = json!({ "tag": tag, "type": "https", "server": host, "server_port": port, "path": path });
            if host.parse::<std::net::IpAddr>().is_err() {
                if !spec.ip.is_empty() {
                    hosts.entry(host.to_lowercase()).or_insert_with(|| spec.ip.clone());
                    v["domain_resolver"] = json!("dns-hosts");
                } else {
                    v["domain_resolver"] = json!("dns-domestic");
                }
            }
            v
        }
        // Through the proxy use TCP: a failed lookup aborts the connection in sing-box's
        // resolve action, and TCP doesn't depend on the server relaying UDP
        _ if detour.is_some() => json!({ "tag": tag, "type": "tcp", "server": spec.ip, "server_port": 53 }),
        _ => json!({ "tag": tag, "type": "udp", "server": spec.ip, "server_port": 53 }),
    };
    if let Some(d) = detour {
        v["detour"] = json!(d);
    }
    v
}

/// Rule items for domain matchers: one item for literal domains, one for geosite rule-sets.
/// Kept as separate rules so sing-box never ANDs them together.
fn domain_items(matchers: &[Matcher], routing: &EffectiveRouting) -> Vec<Value> {
    let (mut full, mut suffix, mut keyword, mut regex, mut sets) = (vec![], vec![], vec![], vec![], vec![]);
    for m in matchers {
        match m {
            Matcher::Full(d) => full.push(d.clone()),
            Matcher::Suffix(d) => suffix.push(d.clone()),
            Matcher::Keyword(d) => keyword.push(d.clone()),
            Matcher::Regex(d) => regex.push(d.clone()),
            Matcher::GeoSite(_) => {
                if let Some(tag) = routing.geo.rule_set_tag(m) {
                    sets.push(tag.to_string());
                }
            }
            _ => {}
        }
    }
    let mut items = Vec::new();
    let mut literal = json!({});
    for (key, list) in [("domain", full), ("domain_suffix", suffix), ("domain_keyword", keyword), ("domain_regex", regex)] {
        if !list.is_empty() {
            literal[key] = json!(list);
        }
    }
    if literal.as_object().map_or(false, |o| !o.is_empty()) {
        items.push(literal);
    }
    if !sets.is_empty() {
        items.push(json!({ "rule_set": sets }));
    }
    items
}

fn ip_items(matchers: &[Matcher], routing: &EffectiveRouting) -> Vec<Value> {
    let mut cidrs = vec![];
    let mut sets = vec![];
    for m in matchers {
        match m {
            Matcher::Cidr(c) => cidrs.push(c.clone()),
            Matcher::GeoIp(_) => {
                if let Some(tag) = routing.geo.rule_set_tag(m) {
                    sets.push(tag.to_string());
                }
            }
            _ => {}
        }
    }
    let mut items = Vec::new();
    if !cidrs.is_empty() {
        items.push(json!({ "ip_cidr": cidrs }));
    }
    if !sets.is_empty() {
        items.push(json!({ "rule_set": sets }));
    }
    items
}

fn set_route_action(item: &mut Value, action: RouteAction) {
    match action {
        RouteAction::Proxy => item["outbound"] = json!("proxy"),
        RouteAction::Direct => item["outbound"] = json!("direct"),
        RouteAction::Block => item["action"] = json!("reject"),
    }
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
        Transport::Xhttp => {
            let xhttp = server.xhttp.as_ref();
            let mut transport = json!({
                "type": "xhttp",
                "path": xhttp.map(|x| x.path.as_str()).unwrap_or("/")
            });
            if let Some(host) = xhttp.and_then(|x| x.host.as_deref()) {
                transport["host"] = json!(host);
            }
            if let Some(mode) = xhttp.and_then(|x| x.mode.as_deref()) {
                transport["mode"] = json!(mode);
            }
            out["transport"] = transport;
        }
        Transport::Httpupgrade => {
            let hu = server.httpupgrade.as_ref();
            let mut transport = json!({
                "type": "httpupgrade",
                "path": hu.map(|h| h.path.as_str()).unwrap_or("/")
            });
            if let Some(host) = hu.and_then(|h| h.host.as_deref()) {
                transport["host"] = json!(host);
            }
            out["transport"] = transport;
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
