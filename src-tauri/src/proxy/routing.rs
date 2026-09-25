//! Routing profiles (Happ-compatible) and the effective rule set fed to the cores.
//!
//! Happ profile format: https://routing.happ.su — delivered as
//! `happ://routing/add/<base64 json>`, `happ://routing/onadd/<base64 json>` (add + activate)
//! or `happ://routing/off`, either as a deep link or in a subscription's `routing` header.

use anyhow::{anyhow, Result};
use base64::Engine;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use super::models::{RoutingProfile, RoutingRule, RuleAction};

pub const DEFAULT_GEOIP_URL: &str =
    "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geoip.dat";
pub const DEFAULT_GEOSITE_URL: &str =
    "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geosite.dat";

const ROUTE_ORDERS: [&str; 6] = [
    "block-proxy-direct",
    "block-direct-proxy",
    "proxy-direct-block",
    "proxy-block-direct",
    "direct-proxy-block",
    "direct-block-proxy",
];

/// Private and link-local ranges that should never go through the proxy.
const PRIVATE_CIDRS: [&str; 8] = [
    "10.0.0.0/8",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "127.0.0.0/8",
    "169.254.0.0/16",
    "::1/128",
    "fc00::/7",
    "fe80::/10",
];

// ── Happ links / JSON ──────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum HappDirective {
    /// Add (or replace by name); becomes active only if nothing else is
    Add(RoutingProfile),
    /// Add (or replace by name) and activate
    OnAdd(RoutingProfile),
    /// Turn profile routing off
    Off,
}

/// Parse a Happ routing link (`happ://` or `nexvpn://` scheme) or a raw JSON profile.
pub fn parse_happ_routing(input: &str) -> Result<HappDirective> {
    let input = input.trim();
    if input.starts_with('{') {
        let json: Value = serde_json::from_str(input)?;
        return Ok(HappDirective::Add(profile_from_happ_json(&json)?));
    }

    let rest = input
        .strip_prefix("happ://routing/")
        .or_else(|| input.strip_prefix("nexvpn://routing/"))
        .ok_or_else(|| anyhow!("Not a routing link (expected happ://routing/...)"))?;

    if rest.trim_end_matches('/') == "off" {
        return Ok(HappDirective::Off);
    }
    let (kind, payload) = rest
        .split_once('/')
        .ok_or_else(|| anyhow!("Malformed routing link"))?;
    let json = decode_payload(payload)?;
    let profile = profile_from_happ_json(&json)?;
    match kind {
        "add" => Ok(HappDirective::Add(profile)),
        "onadd" => Ok(HappDirective::OnAdd(profile)),
        other => Err(anyhow!("Unknown routing link type: {}", other)),
    }
}

fn decode_payload(payload: &str) -> Result<Value> {
    let payload = percent_encoding::percent_decode_str(payload.trim())
        .decode_utf8_lossy()
        .to_string();
    let cleaned: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
    let trimmed = cleaned.trim_end_matches('=');
    let bytes = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(trimmed)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(trimmed))
        .map_err(|e| anyhow!("Routing link is not valid base64: {}", e))?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Build a profile from Happ JSON. Happ writes booleans as strings ("true") and fills
/// missing fields from its default profile; mirror that.
pub fn profile_from_happ_json(json: &Value) -> Result<RoutingProfile> {
    if !json.is_object() {
        return Err(anyhow!("Routing profile must be a JSON object"));
    }
    let s = |key: &str| json.get(key).and_then(value_to_string).unwrap_or_default();
    let b = |key: &str, default: bool| json.get(key).and_then(value_to_bool).unwrap_or(default);
    let list = |key: &str| -> Vec<String> {
        json.get(key)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    };
    let or = |v: String, default: &str| if v.trim().is_empty() { default.to_string() } else { v.trim().to_string() };

    let route_order = s("RouteOrder").to_lowercase();
    let route_order = if ROUTE_ORDERS.contains(&route_order.as_str()) { route_order } else { ROUTE_ORDERS[0].to_string() };

    let dns_hosts: BTreeMap<String, String> = json
        .get("DnsHosts")
        .and_then(|v| v.as_object())
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| value_to_string(v).map(|v| (k.trim().to_lowercase(), v.trim().to_string())))
                .filter(|(k, v)| !k.is_empty() && !v.is_empty())
                .collect()
        })
        .unwrap_or_default();

    Ok(RoutingProfile {
        id: String::new(),
        name: or(s("Name"), "Routing"),
        global_proxy: b("GlobalProxy", true),
        route_order,
        remote_dns_type: normalize_dns_type(&s("RemoteDNSType"), "DoH"),
        remote_dns_domain: s("RemoteDNSDomain"),
        remote_dns_ip: or(s("RemoteDNSIP"), &or(s("RemoteDns"), "1.1.1.1")),
        domestic_dns_type: normalize_dns_type(&s("DomesticDNSType"), "DoU"),
        domestic_dns_domain: s("DomesticDNSDomain"),
        domestic_dns_ip: or(s("DomesticDNSIP"), &or(s("DomesticDns"), "8.8.8.8")),
        geoip_url: or(if s("Geoipurl").is_empty() { s("Geoipturl") } else { s("Geoipurl") }, DEFAULT_GEOIP_URL),
        geosite_url: or(s("Geositeurl"), DEFAULT_GEOSITE_URL),
        last_updated: s("LastUpdated"),
        dns_hosts,
        direct_sites: list("DirectSites"),
        direct_ip: list("DirectIp"),
        proxy_sites: list("ProxySites"),
        proxy_ip: list("ProxyIp"),
        block_sites: list("BlockSites"),
        block_ip: list("BlockIp"),
        domain_strategy: match s("DomainStrategy").as_str() {
            "AsIs" => "AsIs".to_string(),
            "IPOnDemand" => "IPOnDemand".to_string(),
            _ => "IPIfNonMatch".to_string(),
        },
        ..Default::default()
    })
}

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn value_to_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::String(s) => Some(s.trim().eq_ignore_ascii_case("true") || s.trim() == "1"),
        Value::Number(n) => Some(n.as_i64() != Some(0)),
        _ => None,
    }
}

fn normalize_dns_type(t: &str, default: &str) -> String {
    match t.trim().to_ascii_lowercase().as_str() {
        "doh" => "DoH".to_string(),
        "dou" | "udp" => "DoU".to_string(),
        _ => default.to_string(),
    }
}

/// Convert the `routing` block embedded in happ-style JSON subscription configs into a
/// profile. Only plain domain/ip lists with an outboundTag are understood; a catch-all
/// rule (`network: "tcp,udp"`) sets where unmatched traffic goes.
pub fn profile_from_xray_routing(routing: &Value, name: &str) -> Option<RoutingProfile> {
    let rules = routing.get("rules")?.as_array()?;
    let mut profile = RoutingProfile {
        name: name.to_string(),
        global_proxy: true,
        route_order: String::new(),
        remote_dns_type: "DoH".to_string(),
        remote_dns_domain: "https://1.1.1.1/dns-query".to_string(),
        remote_dns_ip: "1.1.1.1".to_string(),
        domestic_dns_type: "DoU".to_string(),
        domestic_dns_ip: "77.88.8.8".to_string(),
        geoip_url: DEFAULT_GEOIP_URL.to_string(),
        geosite_url: DEFAULT_GEOSITE_URL.to_string(),
        domain_strategy: match routing.get("domainStrategy").and_then(|v| v.as_str()) {
            Some("IPIfNonMatch") => "IPIfNonMatch".to_string(),
            Some("IPOnDemand") => "IPOnDemand".to_string(),
            _ => "AsIs".to_string(),
        },
        ..Default::default()
    };
    let mut order: Vec<&str> = Vec::new();
    let mut any = false;
    for rule in rules {
        let action = match rule.get("outboundTag").and_then(|v| v.as_str()) {
            Some("proxy") => "proxy",
            Some("direct") => "direct",
            Some("block") => "block",
            _ => continue,
        };
        let strs = |key: &str| -> Vec<String> {
            rule.get(key)
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default()
        };
        let (domains, ips) = (strs("domain"), strs("ip"));
        if domains.is_empty() && ips.is_empty() {
            if rule.get("network").is_some() && rule.get("port").is_none() {
                profile.global_proxy = action == "proxy";
            }
            continue;
        }
        // Extra conditions would make the rule narrower than a plain list; skip it.
        if ["port", "sourcePort", "source", "inboundTag", "protocol", "user", "attrs"]
            .iter()
            .any(|k| rule.get(*k).is_some())
        {
            continue;
        }
        any = true;
        if !order.contains(&action) {
            order.push(action);
        }
        let (sites, ip_list) = match action {
            "proxy" => (&mut profile.proxy_sites, &mut profile.proxy_ip),
            "direct" => (&mut profile.direct_sites, &mut profile.direct_ip),
            _ => (&mut profile.block_sites, &mut profile.block_ip),
        };
        sites.extend(domains);
        ip_list.extend(ips);
    }
    if !any {
        return None;
    }
    for a in ["block", "proxy", "direct"] {
        if !order.contains(&a) {
            order.push(a);
        }
    }
    profile.route_order = order.join("-");
    Some(profile)
}

// ── Rule entries ───────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Matcher {
    /// Domain and all subdomains
    Suffix(String),
    Full(String),
    Keyword(String),
    Regex(String),
    GeoSite(String),
    /// Normalized CIDR
    Cidr(String),
    GeoIp(String),
}

impl Matcher {
    pub fn is_ip(&self) -> bool {
        matches!(self, Matcher::Cidr(_) | Matcher::GeoIp(_))
    }
}

/// Parse one list entry in Xray syntax. `plain_is_keyword`: Xray treats an unprefixed
/// string as a substring match; our own UI treats it as "domain and subdomains".
pub fn parse_entry(entry: &str, plain_is_keyword: bool) -> Option<Matcher> {
    let e = entry.trim();
    if e.is_empty() {
        return None;
    }
    let lower = e.to_lowercase();
    if let Some(v) = lower.strip_prefix("geosite:") {
        return non_empty(v).map(|v| Matcher::GeoSite(v.to_string()));
    }
    if let Some(v) = lower.strip_prefix("geoip:") {
        // Negated geoip ("geoip:!cn") has no sing-box equivalent
        return non_empty(v).filter(|v| !v.starts_with('!')).map(|v| Matcher::GeoIp(v.to_string()));
    }
    if let Some(v) = e.strip_prefix("regexp:") {
        return non_empty(v).map(|v| Matcher::Regex(v.to_string()));
    }
    if let Some(v) = lower.strip_prefix("full:") {
        return non_empty(v).map(|v| Matcher::Full(v.to_string()));
    }
    if let Some(v) = lower.strip_prefix("domain:") {
        return non_empty(v).map(|v| Matcher::Suffix(v.trim_start_matches('.').to_string()));
    }
    if let Some(v) = lower.strip_prefix("keyword:") {
        return non_empty(v).map(|v| Matcher::Keyword(v.to_string()));
    }
    if let Some(cidr) = normalize_cidr(&lower) {
        return Some(Matcher::Cidr(cidr));
    }
    // ext:file:tag and other file references are not supported
    if lower.starts_with("ext:") || lower.contains(':') {
        return None;
    }
    let plain = lower.trim_start_matches("*.").trim_start_matches('.').to_string();
    Some(if plain_is_keyword { Matcher::Keyword(plain) } else { Matcher::Suffix(plain) })
}

fn non_empty(s: &str) -> Option<&str> {
    let s = s.trim();
    (!s.is_empty()).then_some(s)
}

fn normalize_cidr(s: &str) -> Option<String> {
    if let Some((ip, prefix)) = s.split_once('/') {
        let ip: IpAddr = ip.parse().ok()?;
        let prefix: u8 = prefix.parse().ok()?;
        let max = if ip.is_ipv4() { 32 } else { 128 };
        (prefix <= max).then(|| format!("{}/{}", ip, prefix))
    } else {
        let ip: IpAddr = s.trim_matches(|c| c == '[' || c == ']').parse().ok()?;
        Some(format!("{}/{}", ip, if ip.is_ipv4() { 32 } else { 128 }))
    }
}

// ── Effective routing ──────────────────────────────────

/// Routing settings captured from app state for one core start.
#[derive(Debug, Clone, Default)]
pub struct RoutingInput {
    pub rules: Vec<RoutingRule>,
    pub default_route: String,
    /// The active profile, if any
    pub profile: Option<RoutingProfile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteAction {
    Proxy,
    Direct,
    Block,
}

/// Consecutive entries sharing one action. Groups are evaluated in order.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteGroup {
    pub action: RouteAction,
    pub domains: Vec<Matcher>,
    pub ips: Vec<Matcher>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DnsKind {
    DoH,
    DoU,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DnsSpec {
    pub kind: DnsKind,
    /// DoH URL (DoH only)
    pub url: String,
    pub ip: String,
}

impl DnsSpec {
    fn from_profile(kind: &str, url: &str, ip: &str) -> Self {
        let url = url.trim();
        let doh = kind == "DoH" && url.starts_with("https://");
        DnsSpec {
            kind: if doh { DnsKind::DoH } else { DnsKind::DoU },
            url: if doh { url.to_string() } else { String::new() },
            ip: ip.trim().to_string(),
        }
    }

    /// (host, port, path) of a DoH URL
    pub fn doh_parts(&self) -> Option<(String, u16, String)> {
        let u = url::Url::parse(&self.url).ok()?;
        let host = u.host_str()?.trim_matches(|c| c == '[' || c == ']').to_string();
        let path = if u.path().is_empty() || u.path() == "/" { "/dns-query".to_string() } else { u.path().to_string() };
        Some((host, u.port().unwrap_or(443), path))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileDns {
    /// Queried through the proxy
    pub remote: DnsSpec,
    /// Queried directly
    pub domestic: DnsSpec,
    pub hosts: BTreeMap<String, String>,
}

/// What the geo databases can serve for this run, filled in by the core manager.
#[derive(Debug, Clone, Default)]
pub struct GeoResolution {
    /// Directory holding geosite.dat / geoip.dat (Xray asset dir)
    pub dir: Option<PathBuf>,
    /// Lowercased codes present in the files (Xray refuses to start on unknown codes)
    pub site_codes: HashSet<String>,
    pub ip_codes: HashSet<String>,
    /// sing-box rule-sets: (matcher key "geosite:x" / "geoip:x", tag, path)
    pub rule_sets: Vec<(String, String, PathBuf)>,
}

impl GeoResolution {
    pub fn has_site(&self, code: &str) -> bool {
        self.site_codes.contains(code.split('@').next().unwrap_or(code))
    }
    pub fn has_ip(&self, code: &str) -> bool {
        self.ip_codes.contains(code)
    }
    pub fn rule_set_tag(&self, m: &Matcher) -> Option<&str> {
        let key = match m {
            Matcher::GeoSite(c) => format!("geosite:{}", c),
            Matcher::GeoIp(c) => format!("geoip:{}", c),
            _ => return None,
        };
        self.rule_sets.iter().find(|(k, _, _)| *k == key).map(|(_, t, _)| t.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct EffectiveRouting {
    pub groups: Vec<RouteGroup>,
    /// Unmatched traffic goes via proxy (true) or direct (false)
    pub final_proxy: bool,
    /// Resolve domains that matched no domain rule and retry the IP rules (IPIfNonMatch)
    pub resolve_ips: bool,
    pub dns: Option<ProfileDns>,
    pub geo: GeoResolution,
}

impl EffectiveRouting {
    /// Custom rules first (the user's overrides), then private ranges direct, then the
    /// active profile's lists in its route order.
    pub fn build(custom: &[RoutingRule], default_route: &str, profile: Option<&RoutingProfile>) -> Self {
        let mut groups: Vec<RouteGroup> = Vec::new();
        for rule in custom.iter().filter(|r| r.enabled) {
            let Some(m) = parse_entry(&rule.domain, false) else { continue };
            let action = match rule.action {
                RuleAction::Proxy => RouteAction::Proxy,
                RuleAction::Direct => RouteAction::Direct,
                RuleAction::Block => RouteAction::Block,
            };
            match groups.last_mut() {
                Some(g) if g.action == action => push_matcher(g, m),
                _ => {
                    let mut g = RouteGroup { action, domains: vec![], ips: vec![] };
                    push_matcher(&mut g, m);
                    groups.push(g);
                }
            }
        }

        let custom_has_ip = groups.iter().any(|g| !g.ips.is_empty());

        groups.push(RouteGroup {
            action: RouteAction::Direct,
            domains: vec![],
            ips: PRIVATE_CIDRS.iter().map(|c| Matcher::Cidr(c.to_string())).collect(),
        });

        let (final_proxy, strategy, dns) = match profile {
            Some(p) => {
                for part in p.route_order.split('-') {
                    let (action, sites, ips) = match part {
                        "proxy" => (RouteAction::Proxy, &p.proxy_sites, &p.proxy_ip),
                        "direct" => (RouteAction::Direct, &p.direct_sites, &p.direct_ip),
                        "block" => (RouteAction::Block, &p.block_sites, &p.block_ip),
                        _ => continue,
                    };
                    let mut g = RouteGroup { action, domains: vec![], ips: vec![] };
                    // Happ passes lists to Xray as-is, so unprefixed site entries are keywords
                    for m in sites.iter().filter_map(|e| parse_entry(e, true)) {
                        push_matcher(&mut g, m);
                    }
                    for m in ips.iter().filter_map(|e| parse_entry(e, false)).filter(|m| m.is_ip()) {
                        g.ips.push(m);
                    }
                    if !g.domains.is_empty() || !g.ips.is_empty() {
                        groups.push(g);
                    }
                }
                let dns = ProfileDns {
                    remote: DnsSpec::from_profile(&p.remote_dns_type, &p.remote_dns_domain, &p.remote_dns_ip),
                    domestic: DnsSpec::from_profile(&p.domestic_dns_type, &p.domestic_dns_domain, &p.domestic_dns_ip),
                    hosts: p.dns_hosts.clone(),
                };
                (p.global_proxy, p.domain_strategy.as_str(), Some(dns))
            }
            None => (default_route != "direct", "", None),
        };

        let resolve_ips = match strategy {
            "AsIs" => false,
            "IPIfNonMatch" | "IPOnDemand" => true,
            // No profile: resolve only if the user wrote IP rules of their own
            _ => custom_has_ip,
        };

        EffectiveRouting { groups, final_proxy, resolve_ips, dns, geo: GeoResolution::default() }
    }

    /// Lowercased geosite codes referenced by the rules (with "@attr" suffixes kept)
    pub fn geosite_codes(&self) -> Vec<String> {
        self.collect(|m| match m {
            Matcher::GeoSite(c) => Some(c.clone()),
            _ => None,
        })
    }

    pub fn geoip_codes(&self) -> Vec<String> {
        self.collect(|m| match m {
            Matcher::GeoIp(c) => Some(c.clone()),
            _ => None,
        })
    }

    pub fn needs_geo(&self) -> bool {
        !self.geosite_codes().is_empty() || !self.geoip_codes().is_empty()
    }

    fn collect(&self, f: impl Fn(&Matcher) -> Option<String>) -> Vec<String> {
        let mut seen = HashSet::new();
        self.groups
            .iter()
            .flat_map(|g| g.domains.iter().chain(g.ips.iter()))
            .filter_map(f)
            .filter(|c| seen.insert(c.clone()))
            .collect()
    }

    /// Whether a geo matcher can be used with the current geo files
    pub fn geo_usable_xray(&self, m: &Matcher) -> bool {
        match m {
            Matcher::GeoSite(c) => self.geo.has_site(c),
            Matcher::GeoIp(c) => self.geo.has_ip(c),
            _ => true,
        }
    }
}

fn push_matcher(g: &mut RouteGroup, m: Matcher) {
    if m.is_ip() {
        g.ips.push(m);
    } else {
        g.domains.push(m);
    }
}

/// Where a profile's geo files live: `<geo_root>/<profile id>`; custom rules without a
/// profile use `<geo_root>/default` (Loyalsoldier files).
pub fn geo_dir(geo_root: &Path, profile: Option<&RoutingProfile>) -> PathBuf {
    match profile {
        Some(p) => geo_root.join(sanitize(&p.id)),
        None => geo_root.join("default"),
    }
}

fn sanitize(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROYALTY: &str = "happ://routing/onadd/eyJOYW1lIjoiUm95YWx0eSIsIkdsb2JhbFByb3h5IjoidHJ1ZSIsIlJvdXRlT3JkZXIiOiJibG9jay1wcm94eS1kaXJlY3QiLCJSZW1vdGVETlNUeXBlIjoiRG9VIiwiUmVtb3RlRE5TRG9tYWluIjoiaHR0cHM6Ly8xLjEuMS4xL2Rucy1xdWVyeSIsIlJlbW90ZUROU0lQIjoiMS4xLjEuMSIsIkRvbWVzdGljRE5TVHlwZSI6IkRvVSIsIkRvbWVzdGljRE5TRG9tYWluIjoiaHR0cHM6Ly9zYWZlLmRvdC5kbnMueWFuZGV4Lm5ldCIsIkRvbWVzdGljRE5TSVAiOiI3Ny44OC44LjgiLCJHZW9pcHVybCI6Imh0dHBzOi8vZ2l0aHViLmNvbS9mcmF5WlYvc2ltcGxlLXJ1LWdlb2lwL3JlbGVhc2VzL2xhdGVzdC9kb3dubG9hZC9nZW9pcC5kYXQiLCJHZW9zaXRldXJsIjoiaHR0cHM6Ly9naXRodWIuY29tL2ZyYXlaVi9zaW1wbGUtcnUtZ2Vvc2l0ZS9yZWxlYXNlcy9sYXRlc3QvZG93bmxvYWQvZ2Vvc2l0ZS5kYXQiLCJMYXN0VXBkYXRlZCI6IjEwNzA5MDUyMDAiLCJEbnNIb3N0cyI6e30sIkRpcmVjdFNpdGVzIjpbImdlb3NpdGU6cHJpdmF0ZSIsImdlb3NpdGU6Y2F0ZWdvcnktcnUiLCJnZW9zaXRlOmFwcGxlIiwiZ2Vvc2l0ZTp0d2l0Y2giXSwiRGlyZWN0SXAiOlsiZ2VvaXA6cnUiLCJnZW9pcDpwcml2YXRlIl0sIlByb3h5U2l0ZXMiOlsiZ2Vvc2l0ZTp5b3V0dWJlIl0sIlByb3h5SXAiOltdLCJCbG9ja1NpdGVzIjpbXSwiQmxvY2tJcCI6W10sIkRvbWFpblN0cmF0ZWd5IjoiSVBJZk5vbk1hdGNoIiwiRmFrZUROUyI6ImZhbHNlIiwiVXNlQ2h1bmtGaWxlcyI6ImZhbHNlIn0";

    #[test]
    fn parses_happ_onadd_link() {
        let HappDirective::OnAdd(p) = parse_happ_routing(ROYALTY).unwrap() else { panic!("expected onadd") };
        assert_eq!(p.name, "Royalty");
        assert!(p.global_proxy);
        assert_eq!(p.route_order, "block-proxy-direct");
        assert_eq!(p.remote_dns_type, "DoU");
        assert_eq!(p.domestic_dns_ip, "77.88.8.8");
        assert_eq!(p.direct_sites.len(), 4);
        assert_eq!(p.direct_ip, vec!["geoip:ru", "geoip:private"]);
        assert_eq!(p.proxy_sites, vec!["geosite:youtube"]);
        assert!(p.geosite_url.contains("simple-ru-geosite"));
        assert_eq!(p.domain_strategy, "IPIfNonMatch");
    }

    #[test]
    fn parses_off_and_raw_json() {
        assert_eq!(parse_happ_routing("happ://routing/off").unwrap(), HappDirective::Off);
        let HappDirective::Add(p) = parse_happ_routing(r#"{"Name":"X","GlobalProxy":false,"DirectSites":["vk.com"]}"#).unwrap() else { panic!() };
        assert!(!p.global_proxy);
        assert_eq!(p.geoip_url, DEFAULT_GEOIP_URL);
        assert!(parse_happ_routing("vless://x").is_err());
    }

    #[test]
    fn parses_entries() {
        assert_eq!(parse_entry("geosite:Category-RU", true), Some(Matcher::GeoSite("category-ru".into())));
        assert_eq!(parse_entry("geoip:ru", true), Some(Matcher::GeoIp("ru".into())));
        assert_eq!(parse_entry("tiktok", true), Some(Matcher::Keyword("tiktok".into())));
        assert_eq!(parse_entry("vk.com", false), Some(Matcher::Suffix("vk.com".into())));
        assert_eq!(parse_entry("domain:.vk.com", true), Some(Matcher::Suffix("vk.com".into())));
        assert_eq!(parse_entry("regexp:^Ya\\.ru$", true), Some(Matcher::Regex("^Ya\\.ru$".into())));
        assert_eq!(parse_entry("1.2.3.4", true), Some(Matcher::Cidr("1.2.3.4/32".into())));
        assert_eq!(parse_entry("2001:db8::/32", true), Some(Matcher::Cidr("2001:db8::/32".into())));
        assert_eq!(parse_entry("ext:x.dat:tag", true), None);
        assert_eq!(parse_entry("geoip:!cn", true), None);
    }

    #[test]
    fn builds_groups_in_route_order_after_custom_rules() {
        let HappDirective::OnAdd(p) = parse_happ_routing(ROYALTY).unwrap() else { panic!() };
        let custom = vec![RoutingRule { id: "1".into(), domain: "example.com".into(), action: RuleAction::Proxy, enabled: true }];
        let r = EffectiveRouting::build(&custom, "proxy", Some(&p));
        let actions: Vec<RouteAction> = r.groups.iter().map(|g| g.action).collect();
        // custom proxy, private direct, then block(empty, skipped) → proxy → direct
        assert_eq!(actions, vec![RouteAction::Proxy, RouteAction::Direct, RouteAction::Proxy, RouteAction::Direct]);
        assert!(r.final_proxy && r.resolve_ips);
        assert_eq!(r.geosite_codes(), vec!["youtube", "private", "category-ru", "apple", "twitch"]);
        assert_eq!(r.geoip_codes(), vec!["ru", "private"]);
        assert_eq!(r.dns.unwrap().remote.kind, DnsKind::DoU);
    }

    #[test]
    fn converts_embedded_xray_routing() {
        let routing: Value = serde_json::from_str(r#"{"rules":[
            {"domain":["geosite:youtube","tiktok"],"outboundTag":"proxy"},
            {"ip":["geoip:ru","geoip:private"],"outboundTag":"direct"},
            {"domain":["geosite:category-ru"],"outboundTag":"direct"},
            {"network":"tcp,udp","outboundTag":"proxy"}],"domainStrategy":"IPIfNonMatch"}"#).unwrap();
        let p = profile_from_xray_routing(&routing, "Sub").unwrap();
        assert_eq!(p.route_order, "proxy-direct-block");
        assert_eq!(p.proxy_sites, vec!["geosite:youtube", "tiktok"]);
        assert_eq!(p.direct_ip, vec!["geoip:ru", "geoip:private"]);
        assert!(p.global_proxy);
        assert_eq!(p.domain_strategy, "IPIfNonMatch");
    }

    #[test]
    fn no_profile_uses_default_route() {
        let r = EffectiveRouting::build(&[], "direct", None);
        assert!(!r.final_proxy && !r.resolve_ips && r.dns.is_none() && !r.needs_geo());
    }
}
