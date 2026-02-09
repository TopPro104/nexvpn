use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::Mutex;

#[cfg(target_os = "android")]
use std::os::unix::fs::PermissionsExt;

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
    #[cfg(target_os = "android")]
    fn read_android_paths() -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();
        // Try multiple possible locations for the paths file
        for base in &[
            "/data/data/com.horusvpn.nexvpn/files",
            "/data/user/0/com.horusvpn.nexvpn/files",
        ] {
            let path = PathBuf::from(base).join(".android_paths");
            if let Ok(content) = std::fs::read_to_string(&path) {
                for line in content.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        map.insert(k.trim().to_string(), v.trim().to_string());
                    }
                }
                if !map.is_empty() {
                    log::info!("Read android paths from {}: {:?}", path.display(), map);
                    return map;
                }
            }
        }
        log::warn!("Could not read .android_paths file");
        map
    }

    fn resolve_data_dir() -> PathBuf {
        // Android: use app-private storage
        #[cfg(target_os = "android")]
        {
            // Try known Android app data paths
            for base in &[
                "/data/user/0/com.horusvpn.nexvpn/files",
                "/data/data/com.horusvpn.nexvpn/files",
            ] {
                let dir = PathBuf::from(base).join("nexvpn");
                if std::fs::create_dir_all(&dir).is_ok() {
                    return dir;
                }
            }
        }

        // Desktop: use standard data dir
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nexvpn")
    }

    pub fn new() -> Self {
        let data_dir = Self::resolve_data_dir();
        std::fs::create_dir_all(&data_dir).ok();
        log::info!("CoreManager data_dir: {:?} (exists={})", data_dir, data_dir.exists());

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

    pub async fn start(&self, server: &Server, tun_mode: bool, routing_rules: &[RoutingRule], default_route: &str) -> Result<()> {
        self.stop().await?;

        let core_type = self.core_type.lock().await.clone();
        let socks_port = *self.socks_port.lock().await;
        let http_port = *self.http_port.lock().await;

        // On desktop, TUN is only supported by sing-box (xray has no TUN inbound).
        // On Android, TUN is handled by VpnService + tun2socks, so both cores work.
        #[cfg(not(target_os = "android"))]
        if tun_mode && core_type == CoreType::Xray {
            return Err(anyhow!("TUN mode requires sing-box. Switch core to sing-box in settings."));
        }

        let config = match core_type {
            CoreType::SingBox => singbox::generate_config(server, socks_port, http_port, tun_mode, routing_rules, default_route)?,
            CoreType::Xray => xray::generate_config(server, socks_port, http_port, routing_rules, default_route)?,
        };

        let config_path = self.config_dir.join("running_config.json");
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)
            .map_err(|e| anyhow!("Cannot write config to {}: {}", config_path.display(), e))?;

        log::info!(
            "Starting {:?}{} for '{}' ({}:{})",
            core_type, if tun_mode { " [TUN]" } else { "" },
            server.name, server.address, server.port
        );

        let bin_path = self.resolve_binary(&core_type).await?;

        // On Android: diagnose binary before spawning
        #[cfg(target_os = "android")]
        {
            let meta = std::fs::metadata(&bin_path);
            let exists = bin_path.exists();
            let permissions = meta.as_ref().map(|m| format!("{:o}", m.permissions().mode())).unwrap_or_else(|e| format!("err: {}", e));
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);

            log::info!(
                "Android binary check: path={}, exists={}, size={}, perms={}",
                bin_path.display(), exists, size, permissions
            );

            // Ensure executable
            let chmod = std::process::Command::new("chmod")
                .args(["755", &bin_path.to_string_lossy()])
                .output();
            log::info!("chmod result: {:?}", chmod.map(|o| o.status));

            // Quick test: can we run --version?
            let test = std::process::Command::new(&bin_path)
                .arg("version")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output();
            match &test {
                Ok(o) => log::info!(
                    "Binary test: status={}, stdout={}, stderr={}",
                    o.status,
                    String::from_utf8_lossy(&o.stdout).chars().take(200).collect::<String>(),
                    String::from_utf8_lossy(&o.stderr).chars().take(200).collect::<String>()
                ),
                Err(e) => log::error!("Binary test FAILED: {} (kind={:?})", e, e.kind()),
            }

            // If test failed, return detailed error
            if let Err(e) = &test {
                // Also try to read ELF header
                let header = std::fs::read(&bin_path)
                    .map(|bytes| {
                        if bytes.len() > 4 && &bytes[0..4] == b"\x7fELF" {
                            format!("Valid ELF, {} bytes", bytes.len())
                        } else {
                            format!("NOT ELF! First bytes: {:02x?}", &bytes[..bytes.len().min(16)])
                        }
                    })
                    .unwrap_or_else(|re| format!("cannot read: {}", re));

                return Err(anyhow!(
                    "Cannot execute {}: {} (kind={:?})\nPath: {}\nSize: {} bytes, Perms: {}\nELF: {}",
                    bin_path.file_name().unwrap_or_default().to_string_lossy(),
                    e, e.kind(),
                    bin_path.display(),
                    size, permissions,
                    header
                ));
            }
        }

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
                // Signal Android VPN service to start
                #[cfg(target_os = "android")]
                self.signal_android_vpn(&format!("start:{}", socks_port));
                return Ok(());
            }
        }

        // Even if health check didn't confirm, signal VPN (core may still be starting)
        #[cfg(target_os = "android")]
        self.signal_android_vpn(&format!("start:{}", socks_port));

        log::warn!("SOCKS port not open after health check — core may still be starting");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        // Signal Android VPN service to stop first
        #[cfg(target_os = "android")]
        self.signal_android_vpn("stop");

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

    // ── Android VPN signaling ──────────────────────────
    #[cfg(target_os = "android")]
    fn signal_android_vpn(&self, command: &str) {
        let android_paths = Self::read_android_paths();
        let base_dir = android_paths
            .get("files_dir")
            .map(|s| PathBuf::from(s))
            .unwrap_or_else(|| self.config_dir.clone());

        let cmd_path = base_dir.join("nexvpn/.vpn_command");
        if let Some(parent) = cmd_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        match std::fs::write(&cmd_path, command) {
            Ok(_) => log::info!("VPN signal '{}' written to {}", command, cmd_path.display()),
            Err(e) => log::error!("Failed to write VPN signal: {}", e),
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

        // Android: binaries are packed as .so in jniLibs, find them in native lib dir
        #[cfg(target_os = "android")]
        {
            let so_name = match core_type {
                CoreType::SingBox => "libsingbox.so",
                CoreType::Xray => "libxray.so",
            };

            let mut searched = Vec::new();

            // Read real native lib dir from file written by MainActivity.kt
            let android_paths = Self::read_android_paths();
            let native_lib_dir = android_paths.get("native_lib_dir").cloned();
            let files_dir = android_paths.get("files_dir").cloned();

            // 1) Real native lib dir from Android context
            if let Some(ref lib_dir) = native_lib_dir {
                let p = PathBuf::from(lib_dir).join(so_name);
                searched.push(p.display().to_string());
                if p.exists() {
                    let _ = std::process::Command::new("chmod").args(["755", &p.to_string_lossy()]).output();
                    log::info!("Found {} at {}", so_name, p.display());
                    return Ok(p);
                }
            }

            // 2) Common Android paths
            for base in &[
                "/data/data/com.horusvpn.nexvpn/lib",
                "/data/user/0/com.horusvpn.nexvpn/lib",
            ] {
                let p = PathBuf::from(base).join(so_name);
                searched.push(p.display().to_string());
                if p.exists() {
                    let _ = std::process::Command::new("chmod").args(["755", &p.to_string_lossy()]).output();
                    return Ok(p);
                }
            }

            // 3) Check sidecar_dir
            if let Some(dir) = self.sidecar_dir.lock().await.as_ref() {
                let p = dir.join(so_name);
                searched.push(p.display().to_string());
                if p.exists() {
                    let _ = std::process::Command::new("chmod").args(["755", &p.to_string_lossy()]).output();
                    return Ok(p);
                }
            }

            // 4) config_dir/bin/
            let p = self.config_dir.join("bin").join(so_name);
            searched.push(p.display().to_string());
            if p.exists() {
                let _ = std::process::Command::new("chmod").args(["755", &p.to_string_lossy()]).output();
                return Ok(p);
            }

            // Debug: list native lib dir contents
            let lib_dir_path = native_lib_dir.as_deref().unwrap_or("/data/data/com.horusvpn.nexvpn/lib");
            let lib_contents = std::fs::read_dir(lib_dir_path)
                .map(|entries| {
                    entries.flatten()
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_else(|e| format!("cannot read: {}", e));

            return Err(anyhow!(
                "Core '{}' ({}) not found.\nNativeLibDir: {}\nSearched: {}\nLibDir contents: [{}]",
                name, so_name,
                native_lib_dir.as_deref().unwrap_or("(unknown)"),
                searched.join(", "),
                lib_contents
            ));
        }

        // Desktop platforms
        #[cfg(not(target_os = "android"))]
        {
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
