use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

use super::link_parser;
use super::models::*;
use crate::system::hwid;

/// Decoded metadata from subscription response headers
struct SubHeaderMeta {
    name: Option<String>,
    update_interval: Option<u64>,
    upload: Option<u64>,
    download: Option<u64>,
    total: Option<u64>,
    expire: Option<u64>,
    web_page_url: Option<String>,
    support_url: Option<String>,
    announce: Option<String>,
    refill_date: Option<u64>,
}

/// Decode a header value that may be base64-prefixed ("base64:...") or plain text
fn decode_header_value(val: &str) -> String {
    let val = val.trim();
    if let Some(b64) = val.strip_prefix("base64:") {
        if let Ok(decoded) = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            b64.trim(),
        ) {
            if let Ok(s) = String::from_utf8(decoded) {
                return s.trim().to_string();
            }
        }
    }
    val.to_string()
}

/// Parse Subscription-Userinfo header: "upload=N; download=N; total=N; expire=N"
fn parse_userinfo(header: &str) -> (Option<u64>, Option<u64>, Option<u64>, Option<u64>) {
    let mut upload = None;
    let mut download = None;
    let mut total = None;
    let mut expire = None;
    for part in header.split(';') {
        let part = part.trim();
        if let Some((key, val)) = part.split_once('=') {
            let val = val.trim();
            match key.trim() {
                "upload" => upload = val.parse().ok(),
                "download" => download = val.parse().ok(),
                "total" => total = val.parse().ok(),
                "expire" => expire = val.parse().ok(),
                _ => {}
            }
        }
    }
    (upload, download, total, expire)
}

/// Extract all metadata from subscription response headers
fn extract_header_meta(headers: &reqwest::header::HeaderMap) -> SubHeaderMeta {
    // Name: profile-title → content-disposition filename
    let name = headers
        .get("profile-title")
        .and_then(|v| v.to_str().ok())
        .map(|s| decode_header_value(s))
        .filter(|s| !s.is_empty())
        .or_else(|| {
            headers
                .get("content-disposition")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.find("filename=").map(|pos| s[pos + 9..].trim_matches('"').trim().to_string()))
                .filter(|s| !s.is_empty())
        });

    // Profile-Update-Interval (hours)
    let update_interval = headers
        .get("profile-update-interval")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok());

    // Subscription-Userinfo
    let (upload, download, total, expire) = headers
        .get("subscription-userinfo")
        .and_then(|v| v.to_str().ok())
        .map(|s| parse_userinfo(s))
        .unwrap_or((None, None, None, None));

    // Profile-Web-Page-Url
    let web_page_url = headers
        .get("profile-web-page-url")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // Support-Url
    let support_url = headers
        .get("support-url")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // Announce (may be base64-encoded)
    let announce = headers
        .get("announce")
        .and_then(|v| v.to_str().ok())
        .map(|s| decode_header_value(s))
        .filter(|s| !s.is_empty());

    // Subscription-Refill-Date (unix timestamp)
    let refill_date = headers
        .get("subscription-refill-date")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok());

    SubHeaderMeta {
        name,
        update_interval,
        upload,
        download,
        total,
        expire,
        web_page_url,
        support_url,
        announce,
        refill_date,
    }
}

/// Fetch a subscription URL and parse its contents into servers
pub async fn fetch_subscription(url: &str, name: Option<&str>, hwid_enabled: bool, app_logs: Option<Arc<tokio::sync::Mutex<Vec<String>>>>) -> Result<(Subscription, Vec<Server>)> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(format!("NexVPN/{}", env!("CARGO_PKG_VERSION")))
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

    // Extract all metadata from headers before consuming the response body
    let meta = extract_header_meta(resp.headers());
    let content = resp.text().await?;

    // Log each link parse result to app logs
    if let Some(ref logs) = app_logs {
        let decoded_content = link_parser::decode_subscription_content(&content);
        let mut parsed_count = 0u32;
        let mut skipped_count = 0u32;
        for line in decoded_content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let scheme = line.split("://").next().unwrap_or("?");
            match link_parser::parse_link(line) {
                Ok(s) => {
                    parsed_count += 1;
                    let msg = format!("✓ {} {:?} — {}", scheme, s.protocol, s.name);
                    logs.lock().await.push(msg);
                }
                Err(e) => {
                    skipped_count += 1;
                    let msg = format!("✗ {} — {}", scheme, e);
                    logs.lock().await.push(msg);
                }
            }
        }
        logs.lock().await.push(format!("Subscription: {} parsed, {} skipped", parsed_count, skipped_count));
    }

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
    } else if let Some(n) = meta.name {
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
        update_interval: meta.update_interval,
        upload: meta.upload,
        download: meta.download,
        total: meta.total,
        expire: meta.expire,
        web_page_url: meta.web_page_url,
        support_url: meta.support_url,
        announce: meta.announce,
        refill_date: meta.refill_date,
    };

    log::info!(
        "Fetched subscription '{}': {} servers, update_interval={}h",
        subscription.name,
        servers.len(),
        subscription.update_interval.unwrap_or(0),
    );

    Ok((subscription, servers))
}
