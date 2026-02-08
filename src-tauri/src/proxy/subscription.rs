use anyhow::Result;
use uuid::Uuid;

use super::link_parser;
use super::models::*;
use crate::system::hwid;

/// Extract subscription name from response headers.
/// Priority: profile-title (base64 or plain) → content-disposition filename → None
fn extract_name_from_headers(headers: &reqwest::header::HeaderMap) -> Option<String> {
    // 1. profile-title header (used by most panels)
    if let Some(val) = headers.get("profile-title") {
        if let Ok(s) = val.to_str() {
            let s = s.trim();
            if let Some(b64) = s.strip_prefix("base64:") {
                if let Ok(decoded) = base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    b64.trim(),
                ) {
                    if let Ok(name) = String::from_utf8(decoded) {
                        let name = name.trim().to_string();
                        if !name.is_empty() {
                            return Some(name);
                        }
                    }
                }
            } else if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }

    // 2. content-disposition: attachment; filename=NAME
    if let Some(val) = headers.get("content-disposition") {
        if let Ok(s) = val.to_str() {
            if let Some(pos) = s.find("filename=") {
                let name = s[pos + 9..].trim_matches('"').trim().to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }

    None
}

/// Fetch a subscription URL and parse its contents into servers
pub async fn fetch_subscription(url: &str, name: Option<&str>, hwid_enabled: bool) -> Result<(Subscription, Vec<Server>)> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("NexVPN/1.0")
        .build()?;

    let mut request = client.get(url);

    if hwid_enabled {
        let info = hwid::get_device_info();
        request = request
            .header("x-hwid", &info.hwid)
            .header("x-device-os", &info.platform)
            .header("x-ver-os", &info.os_version)
            .header("x-device-model", &info.model);
    }

    let resp = request.send().await?;

    // Extract name from headers before consuming the response body
    let header_name = extract_name_from_headers(resp.headers());
    let content = resp.text().await?;

    let sub_id = Uuid::new_v4().to_string();
    let mut servers = link_parser::parse_subscription_content(&content);

    // Tag servers with subscription ID
    for server in &mut servers {
        server.subscription_id = Some(sub_id.clone());
    }

    let server_ids: Vec<String> = servers.iter().map(|s| s.id.clone()).collect();

    // Name priority: explicit name → header name → domain from URL
    let resolved_name = if let Some(n) = name.filter(|n| !n.is_empty()) {
        n.to_string()
    } else if let Some(n) = header_name {
        n
    } else {
        url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(String::from))
            .unwrap_or_else(|| format!("Sub {}", &url[..30.min(url.len())]))
    };

    let subscription = Subscription {
        id: sub_id,
        name: resolved_name,
        url: url.to_string(),
        servers: server_ids,
        updated_at: Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        ),
    };

    log::info!(
        "Fetched subscription '{}': {} servers",
        subscription.name,
        servers.len()
    );

    Ok((subscription, servers))
}
