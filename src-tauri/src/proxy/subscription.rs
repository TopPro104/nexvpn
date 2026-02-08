use anyhow::Result;
use uuid::Uuid;

use super::link_parser;
use super::models::*;
use crate::system::hwid;

/// Fetch a subscription URL and parse its contents into servers
pub async fn fetch_subscription(url: &str, name: Option<&str>, hwid_enabled: bool) -> Result<(Subscription, Vec<Server>)> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("NexVPN/0.1")
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
    let content = resp.text().await?;

    let sub_id = Uuid::new_v4().to_string();
    let mut servers = link_parser::parse_subscription_content(&content);

    // Tag servers with subscription ID
    for server in &mut servers {
        server.subscription_id = Some(sub_id.clone());
    }

    let server_ids: Vec<String> = servers.iter().map(|s| s.id.clone()).collect();

    let subscription = Subscription {
        id: sub_id,
        name: name
            .map(String::from)
            .unwrap_or_else(|| format!("Sub {}", &url[..30.min(url.len())])),
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
