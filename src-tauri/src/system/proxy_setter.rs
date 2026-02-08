use anyhow::Result;

/// Enable system-wide HTTP proxy
pub fn set_system_proxy(host: &str, port: u16) -> Result<()> {
    #[cfg(target_os = "windows")]
    windows::set_proxy(host, port)?;

    #[cfg(target_os = "linux")]
    linux::set_proxy(host, port)?;

    #[cfg(target_os = "macos")]
    macos::set_proxy(host, port)?;

    // Android: no-op — VPN uses TUN, not system proxy
    #[cfg(target_os = "android")]
    { let _ = (host, port); }

    Ok(())
}

/// Disable system-wide proxy
pub fn unset_system_proxy() -> Result<()> {
    #[cfg(target_os = "windows")]
    windows::unset_proxy()?;

    #[cfg(target_os = "linux")]
    linux::unset_proxy()?;

    #[cfg(target_os = "macos")]
    macos::unset_proxy()?;

    // Android: no-op
    Ok(())
}

/// Safety: ensure proxy is disabled (call on app exit / panic)
pub fn ensure_proxy_disabled() {
    let _ = unset_system_proxy();
}

// ── Windows ────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod windows {
    use anyhow::Result;

    const REG_PATH: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";

    fn reg_cmd() -> std::process::Command {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = std::process::Command::new("reg");
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }

    pub fn set_proxy(host: &str, port: u16) -> Result<()> {
        let proxy_addr = format!("{}:{}", host, port);
        log::info!("Setting Windows system proxy to {}", proxy_addr);

        // ProxyEnable = 1
        reg_cmd()
            .args(["add", REG_PATH, "/v", "ProxyEnable", "/t", "REG_DWORD", "/d", "1", "/f"])
            .output()?;

        // ProxyServer = host:port
        reg_cmd()
            .args(["add", REG_PATH, "/v", "ProxyServer", "/t", "REG_SZ", "/d", &proxy_addr, "/f"])
            .output()?;

        // Bypass list
        let bypass = "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.20.*;172.21.*;172.22.*;172.23.*;172.24.*;172.25.*;172.26.*;172.27.*;172.28.*;172.29.*;172.30.*;172.31.*;192.168.*;<local>";
        reg_cmd()
            .args(["add", REG_PATH, "/v", "ProxyOverride", "/t", "REG_SZ", "/d", bypass, "/f"])
            .output()?;

        // Notify system of proxy change — use hidden rundll32 instead of powershell
        notify_proxy_change();

        log::info!("Windows system proxy enabled: {}", proxy_addr);
        Ok(())
    }

    pub fn unset_proxy() -> Result<()> {
        log::info!("Disabling Windows system proxy");

        // ProxyEnable = 0
        reg_cmd()
            .args(["add", REG_PATH, "/v", "ProxyEnable", "/t", "REG_DWORD", "/d", "0", "/f"])
            .output()?;

        notify_proxy_change();

        log::info!("Windows system proxy disabled");
        Ok(())
    }

    fn notify_proxy_change() {
        // Direct FFI call to wininet.dll — no subprocess, no console window
        unsafe {
            #[link(name = "wininet")]
            extern "system" {
                fn InternetSetOptionW(
                    hInternet: *mut core::ffi::c_void,
                    dwOption: u32,
                    lpBuffer: *mut core::ffi::c_void,
                    dwBufferLength: u32,
                ) -> i32;
            }

            const INTERNET_OPTION_SETTINGS_CHANGED: u32 = 39;
            const INTERNET_OPTION_REFRESH: u32 = 37;

            InternetSetOptionW(
                core::ptr::null_mut(),
                INTERNET_OPTION_SETTINGS_CHANGED,
                core::ptr::null_mut(),
                0,
            );
            InternetSetOptionW(
                core::ptr::null_mut(),
                INTERNET_OPTION_REFRESH,
                core::ptr::null_mut(),
                0,
            );
        }
    }
}

// ── Linux ──────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod linux {
    use anyhow::Result;
    use std::process::Command;

    pub fn set_proxy(host: &str, port: u16) -> Result<()> {
        let _ = Command::new("gsettings").args(["set", "org.gnome.system.proxy", "mode", "manual"]).output();
        let _ = Command::new("gsettings").args(["set", "org.gnome.system.proxy.http", "host", host]).output();
        let _ = Command::new("gsettings").args(["set", "org.gnome.system.proxy.http", "port", &port.to_string()]).output();
        let _ = Command::new("gsettings").args(["set", "org.gnome.system.proxy.https", "host", host]).output();
        let _ = Command::new("gsettings").args(["set", "org.gnome.system.proxy.https", "port", &port.to_string()]).output();
        Ok(())
    }

    pub fn unset_proxy() -> Result<()> {
        let _ = Command::new("gsettings").args(["set", "org.gnome.system.proxy", "mode", "none"]).output();
        Ok(())
    }
}

// ── macOS ──────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos {
    use anyhow::Result;
    use std::process::Command;

    pub fn set_proxy(host: &str, port: u16) -> Result<()> {
        let port_str = port.to_string();
        let output = Command::new("networksetup").args(["-listallnetworkservices"]).output()?;
        let services = String::from_utf8_lossy(&output.stdout);
        for service in services.lines().skip(1) {
            let service = service.trim();
            if service.is_empty() || service.starts_with('*') { continue; }
            let _ = Command::new("networksetup").args(["-setwebproxy", service, host, &port_str]).output();
            let _ = Command::new("networksetup").args(["-setsecurewebproxy", service, host, &port_str]).output();
        }
        Ok(())
    }

    pub fn unset_proxy() -> Result<()> {
        let output = Command::new("networksetup").args(["-listallnetworkservices"]).output()?;
        let services = String::from_utf8_lossy(&output.stdout);
        for service in services.lines().skip(1) {
            let service = service.trim();
            if service.is_empty() || service.starts_with('*') { continue; }
            let _ = Command::new("networksetup").args(["-setwebproxystate", service, "off"]).output();
            let _ = Command::new("networksetup").args(["-setsecurewebproxystate", service, "off"]).output();
        }
        Ok(())
    }
}
