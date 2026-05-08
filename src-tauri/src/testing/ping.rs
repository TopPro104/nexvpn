use anyhow::Result;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;

use crate::proxy::models::Protocol;

/// TCP connect ping — measures time to establish TCP connection
pub async fn tcp_ping(address: &str, port: u16, timeout_ms: u64) -> Result<u32> {
    let addr = format!("{}:{}", address, port);
    let start = Instant::now();

    timeout(Duration::from_millis(timeout_ms), TcpStream::connect(&addr)).await??;

    let elapsed = start.elapsed().as_millis() as u32;
    Ok(elapsed)
}

/// QUIC ping — sends a minimally-valid QUIC long-header packet with a deliberately
/// unsupported version. Per RFC 9000 §6, the server MUST reply with a Version
/// Negotiation packet, which gives us a clean round-trip measurement without any
/// TLS handshake. Works for Hysteria2 and TUIC (both QUIC-based).
pub async fn quic_ping(address: &str, port: u16, timeout_ms: u64) -> Result<u32> {
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    sock.connect(format!("{}:{}", address, port)).await?;

    // Cheap pseudo-random connection IDs from system time — uniqueness inside a
    // single probe is enough; we don't verify the response contents.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let dcid = nanos.to_le_bytes();
    let scid = nanos.rotate_left(17).to_le_bytes();

    // Long header (0xC0 = header form + fixed bit), 4-byte unsupported version
    // (0x0a0a0a0a is a reserved "greasing" value — guaranteed to be unknown).
    let mut pkt = Vec::with_capacity(1200);
    pkt.push(0xC0);
    pkt.extend_from_slice(&[0x0a, 0x0a, 0x0a, 0x0a]);
    pkt.push(dcid.len() as u8);
    pkt.extend_from_slice(&dcid);
    pkt.push(scid.len() as u8);
    pkt.extend_from_slice(&scid);
    // Pad to 1200 bytes — clients SHOULD send Initials at this size, and some
    // servers drop short packets to avoid amplification.
    pkt.resize(1200, 0);

    let start = Instant::now();
    sock.send(&pkt).await?;

    let mut buf = [0u8; 1500];
    timeout(Duration::from_millis(timeout_ms), sock.recv(&mut buf)).await??;

    Ok(start.elapsed().as_millis() as u32)
}

async fn probe_once(protocol: &Protocol, address: &str, port: u16, timeout_ms: u64) -> Result<u32> {
    match protocol {
        Protocol::Hysteria2 | Protocol::Tuic => quic_ping(address, port, timeout_ms).await,
        _ => tcp_ping(address, port, timeout_ms).await,
    }
}

/// Ping a server 3 times and return average. Uses TCP for stream-based
/// protocols and a QUIC version-negotiation probe for hy2/tuic.
pub async fn ping_average(protocol: &Protocol, address: &str, port: u16) -> Option<u32> {
    let mut results = Vec::new();

    for _ in 0..3 {
        if let Ok(ms) = probe_once(protocol, address, port, 5000).await {
            results.push(ms);
        }
    }

    if results.is_empty() {
        return None;
    }

    let avg = results.iter().sum::<u32>() / results.len() as u32;
    Some(avg)
}

/// Ping multiple servers concurrently, returns vec of (server_id, latency_ms)
pub async fn ping_all(servers: &[(String, Protocol, String, u16)]) -> Vec<(String, Option<u32>)> {
    let mut handles = Vec::new();

    for (id, protocol, address, port) in servers {
        let id = id.clone();
        let protocol = protocol.clone();
        let address = address.clone();
        let port = *port;

        handles.push(tokio::spawn(async move {
            let result = ping_average(&protocol, &address, port).await;
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
