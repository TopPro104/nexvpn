use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::Mutex;

#[cfg(target_os = "android")]
use std::os::unix::fs::PermissionsExt;

use crate::proxy::models::*;
use crate::proxy::routing::{self as routing_mod, EffectiveRouting, GeoResolution, RoutingInput};

use super::{geo, singbox, xray};

pub struct CoreManager {
    process: Arc<Mutex<Option<Child>>>,
    /// sing-box TUN bridge process, used only when core_type==Xray && tun_mode on desktop.
    /// Xray has no native TUN — sing-box handles TUN inbound and forwards to Xray's SOCKS.
    bridge_process: Arc<Mutex<Option<Child>>>,
    config_dir: PathBuf,
    core_type: Arc<Mutex<CoreType>>,
    socks_port: Arc<Mutex<u16>>,
    http_port: Arc<Mutex<u16>>,
    sidecar_dir: Arc<Mutex<Option<PathBuf>>>,
    logs: Arc<Mutex<Vec<String>>>,
    /// Random auth credentials for SOCKS5/HTTP inbounds (regenerated each app launch)
    proxy_auth_user: String,
    proxy_auth_pass: String,
    /// Random secret for Clash API (regenerated each app launch)
    clash_api_secret: String,
    /// Random port for Xray stats API (dokodemo-door), avoids fingerprinting
    xray_api_port: u16,
    /// Random port for Clash API (sing-box), avoids fingerprinting
    clash_api_port: u16,
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

        // Generate random credentials per app launch — prevents other apps
        // from connecting to our local SOCKS5/HTTP/Clash API
        let proxy_auth_user = uuid::Uuid::new_v4().to_string().replace('-', "")[..12].to_string();
        let proxy_auth_pass = uuid::Uuid::new_v4().to_string().replace('-', "");
        let clash_api_secret = uuid::Uuid::new_v4().to_string().replace('-', "");
        // Random ports for internal APIs (avoid fingerprinting)
        let base_seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let xray_api_port = (40000 + (base_seed.wrapping_mul(2862933555777941757) >> 48) % 20000) as u16;
        let clash_api_port = (40000 + (base_seed.wrapping_mul(6364136223846793005) >> 48) % 20000) as u16;

        Self {
            process: Arc::new(Mutex::new(None)),
            bridge_process: Arc::new(Mutex::new(None)),
            config_dir: data_dir,
            core_type: Arc::new(Mutex::new(CoreType::SingBox)),
            socks_port: Arc::new(Mutex::new(10808)),
            http_port: Arc::new(Mutex::new(10809)),
            sidecar_dir: Arc::new(Mutex::new(None)),
            logs: Arc::new(Mutex::new(Vec::new())),
            proxy_auth_user,
            proxy_auth_pass,
            clash_api_secret,
            xray_api_port,
            clash_api_port,
        }
    }

    /// Generate random ports in the safe range (10000-60000)
    pub fn generate_random_ports() -> (u16, u16) {
        let mut rng_seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        // Simple LCG random
        let mut next = move || -> u16 {
            rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let range = 50000u64; // 10000-60000
            (10000 + (rng_seed >> 33) % range) as u16
        };

        let socks = next();
        let mut http = next();
        // Ensure different ports
        while http == socks {
            http = next();
        }
        (socks, http)
    }

    #[allow(dead_code)]
    pub fn proxy_auth(&self) -> (&str, &str) {
        (&self.proxy_auth_user, &self.proxy_auth_pass)
    }

    #[allow(dead_code)]
    pub fn clash_api_secret(&self) -> &str {
        &self.clash_api_secret
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

    #[allow(unused_variables)]
    pub async fn start(&self, server: &Server, tun_mode: bool, routing_input: &RoutingInput, per_app_mode: &str, per_app_list: &[String], stealth_mode: bool) -> Result<()> {
        self.stop().await?;

        let core_type = self.core_type.lock().await.clone();
        let socks_port = *self.socks_port.lock().await;
        let http_port = *self.http_port.lock().await;

        // On Android: TUN is handled by VpnService + tun2socks (both cores work).
        // On desktop: sing-box has native TUN; Xray has none — we spawn sing-box as a
        // TUN→SOCKS bridge in front of Xray (v2rayN / nekoray pattern).
        #[cfg(not(target_os = "android"))]
        let needs_xray_bridge = tun_mode && core_type == CoreType::Xray;
        #[cfg(target_os = "android")]
        let needs_xray_bridge = false;

        // Anything already listening on our ports (e.g. a core orphaned by a previous run)
        // would answer the health check below and make a failed start look connected.
        let (socks_busy, http_busy) = tokio::join!(port_in_use(socks_port), port_in_use(http_port));
        if socks_busy || http_busy {
            return Err(anyhow!(
                "127.0.0.1:{}: address already in use (another program, possibly a leftover sing-box/xray process)",
                if socks_busy { socks_port } else { http_port }
            ));
        }

        let auth = (self.proxy_auth_user.as_str(), self.proxy_auth_pass.as_str());
        let clash_secret = self.clash_api_secret.as_str();

        let mut routing = EffectiveRouting::build(&routing_input.rules, &routing_input.default_route, routing_input.profile.as_ref());
        if routing.needs_geo() {
            let dir = routing_mod::geo_dir(&self.geo_root(), routing_input.profile.as_ref());
            // sing-box can't read .dat files: convert the used categories to rule-sets.
            // The Xray TUN bridge also needs them for its DNS rules.
            let want_rule_sets = core_type == CoreType::SingBox || needs_xray_bridge;
            routing.geo = prepare_geo(dir, &routing, want_rule_sets).await;
        }

        let config = match core_type {
            CoreType::SingBox => singbox::generate_config(server, socks_port, http_port, tun_mode, &routing, auth, clash_secret, self.clash_api_port)?,
            CoreType::Xray => xray::generate_config(server, socks_port, http_port, &routing, auth, self.xray_api_port)?,
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
            CoreType::SingBox => spawn_hidden(&bin_path, &["run", "-c", config_path.to_str().unwrap()], &[])?,
            // Xray looks up geosite.dat / geoip.dat in its asset dir
            CoreType::Xray => {
                let env: Vec<(&str, &std::path::Path)> = routing.geo.dir.iter().map(|d| ("XRAY_LOCATION_ASSET", d.as_path())).collect();
                spawn_hidden(&bin_path, &["-config", config_path.to_str().unwrap()], &env)?
            }
        };

        // Capture stdout/stderr into log buffer
        self.spawn_log_reader(child.stdout.take(), "OUT");
        self.spawn_log_reader(child.stderr.take(), "ERR");
        self.record_pid(&child);

        *self.process.lock().await = Some(child);

        // Optimized health check: progressive intervals, fail fast on process exit
        let delays = [100, 150, 200, 200, 250, 300, 300, 400];
        for (attempt, delay) in delays.iter().enumerate() {
            tokio::time::sleep(std::time::Duration::from_millis(*delay)).await;

            // Check if process died on every iteration (fail fast)
            {
                let mut proc = self.process.lock().await;
                match proc.as_mut() {
                    Some(child) => {
                        if let Ok(Some(status)) = child.try_wait() {
                            *proc = None;
                            return Err(anyhow!("Core exited with status: {}. Check the config.", status));
                        }
                    }
                    // Stopped or reaped while starting — don't report a dead core as connected
                    None => return Err(anyhow!("Core stopped during startup. Check the core logs.")),
                }
            }

            if tokio::net::TcpStream::connect(format!("127.0.0.1:{}", socks_port))
                .await
                .is_ok()
            {
                log::info!("Core started in ~{}ms  SOCKS :{}", delays[..=attempt].iter().sum::<u64>(), socks_port);
                #[cfg(target_os = "android")]
                {
                    self.write_per_app_config(per_app_mode, per_app_list);
                    self.write_stealth_config(stealth_mode);
                    self.signal_android_vpn(&format!("start:{}:{}:{}", socks_port, self.proxy_auth_user, self.proxy_auth_pass));
                }

                #[cfg(not(target_os = "android"))]
                if needs_xray_bridge {
                    self.start_xray_tun_bridge(socks_port, &server.address, &bin_path, &routing).await?;
                }
                #[cfg(target_os = "android")]
                let _ = needs_xray_bridge;

                return Ok(());
            }
        }

        // Even if health check didn't confirm, signal VPN (core may still be starting)
        #[cfg(target_os = "android")]
        {
            self.write_per_app_config(per_app_mode, per_app_list);
            self.write_stealth_config(stealth_mode);
            self.signal_android_vpn(&format!("start:{}:{}:{}", socks_port, self.proxy_auth_user, self.proxy_auth_pass));
        }

        log::warn!("SOCKS port not open after health check — core may still be starting");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        // Signal Android VPN service to stop first
        #[cfg(target_os = "android")]
        self.signal_android_vpn("stop");

        // Stop sing-box bridge before main core so TUN routes clear while SOCKS is still up
        if let Some(mut child) = self.bridge_process.lock().await.take() {
            log::info!("Stopping sing-box TUN bridge");
            child.kill().await.ok();
            child.wait().await.ok();
        }

        if let Some(mut child) = self.process.lock().await.take() {
            log::info!("Stopping core process");
            child.kill().await.ok();
            child.wait().await.ok();
        }
        let _ = std::fs::remove_file(self.pid_file());
        Ok(())
    }

    /// Root folder for downloaded geo files (one subfolder per routing profile)
    pub fn geo_root(&self) -> PathBuf {
        self.config_dir.join("geo")
    }

    // ── Orphaned core cleanup ──────────────────────────

    fn pid_file(&self) -> PathBuf {
        self.config_dir.join("core.pids")
    }

    /// Remember a spawned core's PID so the next launch can kill it if this instance
    /// dies without calling stop() (crash, force-kill from Task Manager).
    fn record_pid(&self, child: &Child) {
        use std::io::Write;
        if let Some(pid) = child.id() {
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(self.pid_file()) {
                let _ = writeln!(f, "{}", pid);
            }
        }
    }

    /// Kill cores left running by a previous instance. Only PIDs we recorded that still
    /// belong to a sing-box/xray process are touched, so a reused PID is never killed.
    #[cfg(not(target_os = "android"))]
    pub fn kill_orphaned_cores(&self) {
        let path = self.pid_file();
        let Ok(content) = std::fs::read_to_string(&path) else { return };
        for pid in content.lines().filter_map(|l| l.trim().parse::<u32>().ok()) {
            if !is_core_process(pid) {
                continue;
            }
            log::warn!("Killing core process left over from a previous run (PID {})", pid);
            if !kill_process(pid) {
                log::error!(
                    "Cannot kill leftover core process (PID {}); it may be running as administrator. End sing-box/xray in Task Manager.",
                    pid
                );
            }
        }
        let _ = std::fs::remove_file(&path);
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

    /// Write stealth mode flag for Android VpnService
    #[cfg(target_os = "android")]
    fn write_stealth_config(&self, enabled: bool) {
        let android_paths = Self::read_android_paths();
        let base_dir = android_paths
            .get("files_dir")
            .map(|s| PathBuf::from(s))
            .unwrap_or_else(|| self.config_dir.clone());

        let nexvpn_dir = base_dir.join("nexvpn");
        std::fs::create_dir_all(&nexvpn_dir).ok();

        let path = nexvpn_dir.join(".stealth_mode");
        std::fs::write(&path, if enabled { "true" } else { "false" }).ok();
        log::info!("Stealth mode config written: {}", enabled);
    }

    /// Write per-app VPN config file for Android VpnService to read
    #[cfg(target_os = "android")]
    fn write_per_app_config(&self, mode: &str, apps: &[String]) {
        let android_paths = Self::read_android_paths();
        let base_dir = android_paths
            .get("files_dir")
            .map(|s| PathBuf::from(s))
            .unwrap_or_else(|| self.config_dir.clone());

        let nexvpn_dir = base_dir.join("nexvpn");
        std::fs::create_dir_all(&nexvpn_dir).ok();

        // Format: first line = mode, rest = package names
        let content = format!("{}\n{}", mode, apps.join("\n"));
        let path = nexvpn_dir.join(".per_app_config");
        std::fs::write(&path, content).ok();
        log::info!("Per-app config written: mode={}, apps={}", mode, apps.len());
    }

    #[cfg(target_os = "android")]
    fn signal_android_vpn(&self, command: &str) {
        let android_paths = Self::read_android_paths();
        let base_dir = android_paths
            .get("files_dir")
            .map(|s| PathBuf::from(s))
            .unwrap_or_else(|| self.config_dir.clone());

        let nexvpn_dir = base_dir.join("nexvpn");
        std::fs::create_dir_all(&nexvpn_dir).ok();

        let cmd_path = nexvpn_dir.join(".vpn_command");
        match std::fs::write(&cmd_path, command) {
            Ok(_) => log::info!("VPN signal '{}' written to {}", command, cmd_path.display()),
            Err(e) => log::error!("Failed to write VPN signal: {}", e),
        }

        // Also write .vpn_status so the Quick Settings Tile can read the real state
        let status = if command == "stop" { "stopped" } else { "running" };
        let status_path = nexvpn_dir.join(".vpn_status");
        std::fs::write(&status_path, status).ok();
    }

    // ── sing-box TUN bridge for Xray on desktop ──────────

    /// Spawn sing-box as a TUN→SOCKS bridge in front of Xray.
    /// Called only after Xray's SOCKS inbound is confirmed reachable.
    #[cfg(not(target_os = "android"))]
    async fn start_xray_tun_bridge(&self, xray_socks_port: u16, server_address: &str, xray_bin: &std::path::Path, routing: &EffectiveRouting) -> Result<()> {
        // Windows: remove stale Wintun device so sing-box can create a fresh one.
        // Without this, a crashed previous run leaves a zombie adapter that blocks startup.
        #[cfg(target_os = "windows")]
        Self::cleanup_wintun_device("nexvpn-tun");

        let bridge_bin = self.resolve_binary(&CoreType::SingBox).await
            .map_err(|e| anyhow!("TUN mode with Xray requires sing-box binary alongside the app: {}", e))?;

        let xray_process = xray_bin.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "xray".to_string());
        let config = super::singbox::generate_bridge_config(xray_socks_port, server_address, &xray_process, routing);
        let config_path = self.config_dir.join("running_bridge.json");
        std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)
            .map_err(|e| anyhow!("Cannot write bridge config: {}", e))?;

        log::info!("Starting sing-box TUN bridge → SOCKS 127.0.0.1:{}", xray_socks_port);

        let mut child = spawn_hidden(&bridge_bin, &["run", "-c", config_path.to_str().unwrap()], &[])?;
        self.spawn_log_reader(child.stdout.take(), "BRIDGE-OUT");
        self.spawn_log_reader(child.stderr.take(), "BRIDGE-ERR");
        self.record_pid(&child);

        // Short health check: if bridge exits within 1s, it failed (usually lack of admin rights)
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        if let Ok(Some(status)) = child.try_wait() {
            return Err(anyhow!(
                "TUN bridge (sing-box) exited with status {}. TUN mode typically requires administrator/root privileges.",
                status
            ));
        }

        *self.bridge_process.lock().await = Some(child);
        Ok(())
    }

    /// Remove a leftover Wintun adapter by name. Uses pnputil with a GUID derived from
    /// MD5(interface_name) — this matches v2rayN's scheme since sing-box uses the same.
    #[cfg(target_os = "windows")]
    fn cleanup_wintun_device(interface_name: &str) {
        // sing-box names wintun adapters with a prefix — also try the raw name
        for name in &[format!("wintun{}", interface_name), interface_name.to_string()] {
            let digest = md5::compute(name.as_bytes());
            let b = digest.0;
            let guid = format!(
                "{{{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}}}",
                b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
                b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
            );
            let arg = format!("SWD\\Wintun\\{}", guid);
            let _ = std::process::Command::new(r"C:\Windows\System32\pnputil.exe")
                .args(["/remove-device", &arg])
                .output();
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
                let mut reality_hint_shown = false;
                while let Ok(Some(line)) = lines.next_line().await {
                    // Xray logs this for every rejected connection; explain it once per core run.
                    if !reality_hint_shown && line.contains("REALITY: received real certificate") {
                        reality_hint_shown = true;
                        log::warn!(
                            "REALITY handshake rejected: the server did not recognise this client and \
                             answered as the real SNI site. Check that pbk/sid/sni match the server \
                             (update the subscription), that the system clock is correct, or try \
                             another network — the ISP may be redirecting the connection."
                        );
                    }
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
            .get(format!("http://127.0.0.1:{}/connections", self.clash_api_port))
            .header("Authorization", format!("Bearer {}", self.clash_api_secret))
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
        cmd.args(["api", "statsquery", &format!("--server=127.0.0.1:{}", self.xray_api_port)])
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

            // 0) NEXVPN_SIDECAR_DIR env var (Flatpak / custom installs)
            if let Ok(dir) = std::env::var("NEXVPN_SIDECAR_DIR") {
                let p = PathBuf::from(&dir).join(format!("{}{}", name, exe_ext));
                if p.exists() { return Ok(p); }
            }

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

/// Read which geo codes exist and (for sing-box) convert the used ones to rule-sets.
/// Never fails: rules whose data is unavailable are dropped with a warning, so a missing
/// or broken geo file degrades routing instead of preventing the connection.
async fn prepare_geo(dir: PathBuf, routing: &EffectiveRouting, want_rule_sets: bool) -> GeoResolution {
    let site_codes_wanted = routing.geosite_codes();
    let ip_codes_wanted = routing.geoip_codes();
    let result = tokio::task::spawn_blocking(move || -> Result<GeoResolution> {
        let site_dat = dir.join("geosite.dat");
        let ip_dat = dir.join("geoip.dat");
        let mut res = GeoResolution { dir: Some(dir.clone()), ..Default::default() };
        if !site_codes_wanted.is_empty() && site_dat.exists() {
            res.site_codes = geo::list_codes(&site_dat)?;
        }
        if !ip_codes_wanted.is_empty() && ip_dat.exists() {
            res.ip_codes = geo::list_codes(&ip_dat)?;
        }
        let sites: Vec<String> = site_codes_wanted.into_iter().filter(|c| res.has_site(c)).collect();
        let ips: Vec<String> = ip_codes_wanted.into_iter().filter(|c| res.has_ip(c)).collect();
        if want_rule_sets && (!sites.is_empty() || !ips.is_empty()) {
            let built = geo::build_singbox_rule_sets(
                (!sites.is_empty()).then_some(site_dat.as_path()),
                (!ips.is_empty()).then_some(ip_dat.as_path()),
                &sites,
                &ips,
                &dir.join("rule-sets"),
            )?;
            let keyed = sites
                .iter()
                .map(|c| (format!("geosite:{}", c), geo::geosite_tag(c)))
                .chain(ips.iter().map(|c| (format!("geoip:{}", c), geo::geoip_tag(c))));
            for (key, tag) in keyed {
                if let Some((_, path)) = built.iter().find(|(t, _)| *t == tag) {
                    res.rule_sets.push((key, tag, path.clone()));
                }
            }
        }
        Ok(res)
    })
    .await;

    let res = match result {
        Ok(Ok(res)) => res,
        Ok(Err(e)) => {
            log::warn!("Geo data unavailable, geosite/geoip rules skipped: {}", e);
            GeoResolution::default()
        }
        Err(e) => {
            log::warn!("Geo preparation panicked: {}", e);
            GeoResolution::default()
        }
    };
    let missing: Vec<String> = routing
        .geosite_codes()
        .into_iter()
        .filter(|c| !res.has_site(c))
        .map(|c| format!("geosite:{}", c))
        .chain(routing.geoip_codes().into_iter().filter(|c| !res.has_ip(c)).map(|c| format!("geoip:{}", c)))
        .collect();
    if !missing.is_empty() {
        log::warn!("Routing entries skipped (not in geo files or files not downloaded): {}", missing.join(", "));
    }
    res
}

/// True if something already accepts connections on 127.0.0.1:port.
/// A live listener answers in well under the timeout; the cap matters on Windows,
/// where a refused localhost connect takes ~2s of SYN retries.
async fn port_in_use(port: u16) -> bool {
    matches!(
        tokio::time::timeout(
            std::time::Duration::from_millis(150),
            tokio::net::TcpStream::connect(("127.0.0.1", port)),
        )
        .await,
        Ok(Ok(_))
    )
}

/// Whether `pid` is a running sing-box/xray process.
#[cfg(target_os = "windows")]
fn is_core_process(pid: u32) -> bool {
    use std::os::windows::process::CommandExt;
    // CSV row: "sing-box.exe","1234","Console","1","12,345 K"
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()
        .map(|o| {
            let out = String::from_utf8_lossy(&o.stdout).trim_start().to_lowercase();
            out.starts_with("\"sing-box") || out.starts_with("\"xray")
        })
        .unwrap_or(false)
}

#[cfg(all(unix, not(target_os = "android")))]
fn is_core_process(pid: u32) -> bool {
    // comm= is the bare name on Linux (truncated to 15 chars) and the full path on macOS
    std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .map(|o| {
            let out = String::from_utf8_lossy(&o.stdout);
            let name = out.trim().rsplit('/').next().unwrap_or("");
            name.starts_with("sing-box") || name.starts_with("xray")
        })
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn kill_process(pid: u32) -> bool {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(all(unix, not(target_os = "android")))]
fn kill_process(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Spawn a process with hidden console window on Windows
#[allow(unused_imports)]
fn spawn_hidden(bin: &PathBuf, args: &[&str], env: &[(&str, &std::path::Path)]) -> Result<Child> {
    let mut cmd = tokio::process::Command::new(bin);
    for (key, val) in env {
        cmd.env(key, val);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::routing::{parse_happ_routing, HappDirective};

    /// Writes real configs for manual checks with the actual cores:
    /// NEXVPN_E2E_GEO=<dir with geosite.dat/geoip.dat> NEXVPN_E2E_OUT=<dir>
    /// NEXVPN_E2E_PROFILE=<happ link> cargo test --lib write_e2e_configs -- --ignored
    #[tokio::test]
    #[ignore]
    async fn write_e2e_configs() {
        let geo_dir = PathBuf::from(std::env::var("NEXVPN_E2E_GEO").unwrap());
        let out = PathBuf::from(std::env::var("NEXVPN_E2E_OUT").unwrap());
        let link = std::env::var("NEXVPN_E2E_PROFILE").unwrap();
        let (HappDirective::OnAdd(mut profile) | HappDirective::Add(mut profile)) = parse_happ_routing(&link).unwrap() else { panic!() };
        profile.id = "e2e".into();
        let server = crate::proxy::link_parser::parse_link(
            "vless://00000000-0000-0000-0000-000000000000@203.0.113.10:443?security=reality&pbk=Z84J2IelR9ch3k8VtlVhhs5ycBUlXA7wHBWcBrjqnAw&sid=ab&sni=www.example.com&fp=chrome&type=tcp&flow=xtls-rprx-vision#e2e",
        ).unwrap();
        let custom = vec![
            RoutingRule { id: "1".into(), domain: "example.org".into(), action: RuleAction::Direct, enabled: true },
            RoutingRule { id: "2".into(), domain: "geosite:youtube".into(), action: RuleAction::Block, enabled: false },
        ];
        let mut routing = EffectiveRouting::build(&custom, "proxy", Some(&profile));
        routing.geo = prepare_geo(geo_dir, &routing, true).await;
        std::fs::create_dir_all(&out).unwrap();
        let write = |name: &str, v: &serde_json::Value| std::fs::write(out.join(name), serde_json::to_string_pretty(v).unwrap()).unwrap();
        let auth = ("u", "p");
        write("singbox-proxy.json", &singbox::generate_config(&server, 21080, 21081, false, &routing, auth, "s", 21090).unwrap());
        write("singbox-tun.json", &singbox::generate_config(&server, 21080, 21081, true, &routing, auth, "s", 21090).unwrap());
        write("xray.json", &xray::generate_config(&server, 21080, 21081, &routing, auth, 21091).unwrap());
        write("bridge.json", &singbox::generate_bridge_config(21080, &server.address, "xray", &routing));
        let no_profile = EffectiveRouting::build(&custom, "direct", None);
        write("singbox-noprofile.json", &singbox::generate_config(&server, 21080, 21081, false, &no_profile, auth, "s", 21090).unwrap());
        write("xray-noprofile.json", &xray::generate_config(&server, 21080, 21081, &no_profile, auth, 21091).unwrap());
    }
}
