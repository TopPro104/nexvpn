use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::Mutex;

use crate::proxy::models::*;

use super::{singbox, xray};

pub struct CoreManager {
    process: Arc<Mutex<Option<Child>>>,
    config_dir: PathBuf,
    core_type: Arc<Mutex<CoreType>>,
    socks_port: Arc<Mutex<u16>>,
    http_port: Arc<Mutex<u16>>,
    sidecar_dir: Arc<Mutex<Option<PathBuf>>>,
    logs: Arc<Mutex<Vec<String>>>,
}

impl CoreManager {
    pub fn new() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nexvpn");
        std::fs::create_dir_all(&data_dir).ok();

        Self {
            process: Arc::new(Mutex::new(None)),
            config_dir: data_dir,
            core_type: Arc::new(Mutex::new(CoreType::SingBox)),
            socks_port: Arc::new(Mutex::new(10808)),
            http_port: Arc::new(Mutex::new(10809)),
            sidecar_dir: Arc::new(Mutex::new(None)),
            logs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn set_sidecar_dir(&self, dir: PathBuf) {
        *self.sidecar_dir.lock().await = Some(dir);
    }

    pub async fn set_core_type(&self, core_type: CoreType) {
        *self.core_type.lock().await = core_type;
    }

    pub async fn get_core_type(&self) -> CoreType {
        self.core_type.lock().await.clone()
    }

    pub async fn set_ports(&self, socks: u16, http: u16) {
        *self.socks_port.lock().await = socks;
        *self.http_port.lock().await = http;
    }

    pub async fn socks_port(&self) -> u16 {
        *self.socks_port.lock().await
    }

    pub async fn http_port(&self) -> u16 {
        *self.http_port.lock().await
    }

    pub async fn start(&self, server: &Server, tun_mode: bool) -> Result<()> {
        self.stop().await?;

        let core_type = self.core_type.lock().await.clone();
        let socks_port = *self.socks_port.lock().await;
        let http_port = *self.http_port.lock().await;

        if tun_mode && core_type == CoreType::Xray {
            return Err(anyhow!("TUN mode requires sing-box. Switch core to sing-box in settings."));
        }

        let config = match core_type {
            CoreType::SingBox => singbox::generate_config(server, socks_port, http_port, tun_mode)?,
            CoreType::Xray => xray::generate_config(server, socks_port, http_port)?,
        };

        let config_path = self.config_dir.join("running_config.json");
        std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

        log::info!(
            "Starting {:?}{} for '{}' ({}:{})",
            core_type, if tun_mode { " [TUN]" } else { "" },
            server.name, server.address, server.port
        );

        let bin_path = self.resolve_binary(&core_type).await?;

        // Clear logs from previous session
        self.logs.lock().await.clear();

        let mut child = match core_type {
            CoreType::SingBox => spawn_hidden(&bin_path, &["run", "-c", config_path.to_str().unwrap()])?,
            CoreType::Xray => spawn_hidden(&bin_path, &["-config", config_path.to_str().unwrap()])?,
        };

        // Capture stdout/stderr into log buffer
        self.spawn_log_reader(child.stdout.take(), "OUT");
        self.spawn_log_reader(child.stderr.take(), "ERR");

        *self.process.lock().await = Some(child);

        // Optimized health check: progressive intervals, fail fast on process exit
        let delays = [100, 150, 200, 200, 250, 300, 300, 400];
        for (attempt, delay) in delays.iter().enumerate() {
            tokio::time::sleep(std::time::Duration::from_millis(*delay)).await;

            // Check if process died on every iteration (fail fast)
            {
                let mut proc = self.process.lock().await;
                if let Some(ref mut child) = *proc {
                    if let Ok(Some(status)) = child.try_wait() {
                        *proc = None;
                        return Err(anyhow!("Core exited with status: {}. Check the config.", status));
                    }
                }
            }

            if tokio::net::TcpStream::connect(format!("127.0.0.1:{}", socks_port))
                .await
                .is_ok()
            {
                log::info!("Core started in ~{}ms  SOCKS :{}", delays[..=attempt].iter().sum::<u64>(), socks_port);
                return Ok(());
            }
        }

        log::warn!("SOCKS port not open after health check — core may still be starting");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        if let Some(mut child) = self.process.lock().await.take() {
            log::info!("Stopping core process");
            child.kill().await.ok();
            child.wait().await.ok();
        }
        Ok(())
    }

    pub async fn is_running(&self) -> bool {
        let mut proc = self.process.lock().await;
        if let Some(ref mut child) = *proc {
            match child.try_wait() {
                Ok(Some(_)) => { *proc = None; false }
                Ok(None) => true,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    // ── Logs ────────────────────────────────────────

    pub async fn get_logs(&self) -> Vec<String> {
        self.logs.lock().await.clone()
    }

    pub async fn clear_logs(&self) {
        self.logs.lock().await.clear()
    }

    fn spawn_log_reader<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
        &self,
        reader: Option<R>,
        _tag: &str,
    ) {
        if let Some(reader) = reader {
            let logs = self.logs.clone();
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut lines = BufReader::new(reader).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut buf = logs.lock().await;
                    buf.push(line);
                    let blen = buf.len();
                    if blen > 2000 {
                        buf.drain(0..blen - 1500);
                    }
                }
            });
        }
    }

    /// Get traffic stats from running core (never fails — returns zeros on error)
    pub async fn get_traffic_stats(&self) -> TrafficStats {
        let core_type = self.core_type.lock().await.clone();
        let result = match core_type {
            CoreType::SingBox => self.get_singbox_traffic().await,
            CoreType::Xray => self.get_xray_traffic().await,
        };
        match result {
            Ok(stats) => stats,
            Err(e) => {
                log::debug!("Traffic stats unavailable: {}", e);
                TrafficStats::default()
            }
        }
    }

    async fn get_singbox_traffic(&self) -> Result<TrafficStats> {
        // CRITICAL: .no_proxy() — otherwise reqwest uses system proxy
        // which points to our own VPN proxy, causing a loop/timeout
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(2))
            .build()?;

        let resp = client
            .get("http://127.0.0.1:9090/connections")
            .send()
            .await?;

        let data: serde_json::Value = resp.json().await?;

        // Clash API compat: camelCase fields
        let upload = data["uploadTotal"].as_u64()
            .or_else(|| data["upload_total"].as_u64())
            .unwrap_or(0);
        let download = data["downloadTotal"].as_u64()
            .or_else(|| data["download_total"].as_u64())
            .unwrap_or(0);

        Ok(TrafficStats { upload, download })
    }

    async fn get_xray_traffic(&self) -> Result<TrafficStats> {
        // Xray stats API is gRPC — use the xray binary to query it
        let bin = self.resolve_binary(&CoreType::Xray).await?;

        let mut cmd = tokio::process::Command::new(&bin);
        cmd.args(["api", "statsquery", "--server=127.0.0.1:10813"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

        #[cfg(target_os = "windows")]
        {
            #[allow(unused_imports)]
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let output = cmd.output().await?;

        if !output.status.success() {
            return Ok(TrafficStats::default());
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let data: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();

        let mut upload = 0u64;
        let mut download = 0u64;

        if let Some(stats) = data["stat"].as_array() {
            for stat in stats {
                let name = stat["name"].as_str().unwrap_or("");
                let value = stat["value"]
                    .as_str()
                    .and_then(|v| v.parse::<u64>().ok())
                    .or_else(|| stat["value"].as_u64())
                    .unwrap_or(0);

                if name.contains("uplink") {
                    upload += value;
                } else if name.contains("downlink") {
                    download += value;
                }
            }
        }

        Ok(TrafficStats { upload, download })
    }

    // ── Binary resolution ──────────────────────────────

    async fn resolve_binary(&self, core_type: &CoreType) -> Result<PathBuf> {
        let name = match core_type {
            CoreType::SingBox => "sing-box",
            CoreType::Xray => "xray",
        };
        let exe_ext = if cfg!(windows) { ".exe" } else { "" };

        // 1) Tauri sidecar dir
        if let Some(dir) = self.sidecar_dir.lock().await.as_ref() {
            let p = dir.join(format!("{}{}", name, exe_ext));
            if p.exists() { return Ok(p); }
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let fname = entry.file_name().to_string_lossy().to_string();
                    if fname.starts_with(name) && entry.path().is_file() {
                        return Ok(entry.path());
                    }
                }
            }
        }

        // 2) Next to exe
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let p = dir.join(format!("{}{}", name, exe_ext));
                if p.exists() { return Ok(p); }
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let fname = entry.file_name().to_string_lossy().to_string();
                        if fname.starts_with(name) && entry.path().is_file() {
                            return Ok(entry.path());
                        }
                    }
                }
            }
        }

        // 3) Data dir
        let p = self.config_dir.join("bin").join(format!("{}{}", name, exe_ext));
        if p.exists() { return Ok(p); }

        // 4) PATH
        let which = if cfg!(windows) { "where" } else { "which" };
        if let Ok(out) = std::process::Command::new(which).arg(name).output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if let Some(first) = s.lines().next() {
                    let p = PathBuf::from(first);
                    if p.exists() { return Ok(p); }
                }
            }
        }

        Err(anyhow!(
            "Core binary '{}' not found. Place it next to NexVPN.exe or in PATH.\n\
             Download: sing-box → github.com/SagerNet/sing-box/releases\n\
             Download: xray → github.com/XTLS/Xray-core/releases",
            name
        ))
    }
}

/// Spawn a process with hidden console window on Windows
#[allow(unused_imports)]
fn spawn_hidden(bin: &PathBuf, args: &[&str]) -> Result<Child> {
    let mut cmd = tokio::process::Command::new(bin);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    // On Windows, prevent console window from appearing
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    Ok(cmd.spawn()?)
}
