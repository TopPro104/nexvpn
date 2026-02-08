use anyhow::Result;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// TCP connect ping — measures time to establish TCP connection
pub async fn tcp_ping(address: &str, port: u16, timeout_ms: u64) -> Result<u32> {
    let addr = format!("{}:{}", address, port);
    let start = Instant::now();

    timeout(Duration::from_millis(timeout_ms), TcpStream::connect(&addr)).await??;

    let elapsed = start.elapsed().as_millis() as u32;
    Ok(elapsed)
}

/// Ping a server 3 times and return average
pub async fn ping_average(address: &str, port: u16) -> Option<u32> {
    let mut results = Vec::new();

    for _ in 0..3 {
        match tcp_ping(address, port, 5000).await {
            Ok(ms) => results.push(ms),
            Err(_) => {}
        }
    }

    if results.is_empty() {
        return None;
    }

    let avg = results.iter().sum::<u32>() / results.len() as u32;
    Some(avg)
}

/// Ping multiple servers concurrently, returns vec of (server_id, latency_ms)
pub async fn ping_all(servers: &[(String, String, u16)]) -> Vec<(String, Option<u32>)> {
    let mut handles = Vec::new();

    for (id, address, port) in servers {
        let id = id.clone();
        let address = address.clone();
        let port = *port;

        handles.push(tokio::spawn(async move {
            let result = ping_average(&address, port).await;
            (id, result)
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        if let Ok(result) = handle.await {
            results.push(result);
        }
    }

    results
}
