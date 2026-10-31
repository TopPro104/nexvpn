//! Detects running DPI-bypass tools (zapret, GoodbyeDPI). They rewrite or inject packets
//! with WinDivert / NFQUEUE, and that breaks the VPN's own connection to its server.

/// Names of the tools that are active right now (empty if none).
#[cfg(target_os = "windows")]
pub fn detect() -> Vec<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let mut found: Vec<String> = Vec::new();
    let add = |found: &mut Vec<String>, name: &str| {
        if !found.iter().any(|f| f == name) {
            found.push(name.to_string());
        }
    };

    // CSV rows: "winws.exe","1234",...
    let processes = std::process::Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase())
        .unwrap_or_default();
    if processes.contains("\"winws.exe\"") {
        add(&mut found, "zapret");
    }
    if processes.contains("\"goodbyedpi.exe\"") {
        add(&mut found, "GoodbyeDPI");
    }

    // zapret's service.bat installs "zapret"; both tools load the WinDivert driver.
    // The state keyword (RUNNING) stays English in localized sc output.
    let running = |service: &str| {
        std::process::Command::new("sc")
            .args(["query", service])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("RUNNING"))
            .unwrap_or(false)
    };
    if running("zapret") {
        add(&mut found, "zapret");
    }
    if running("GoodbyeDPI") {
        add(&mut found, "GoodbyeDPI");
    }
    if found.is_empty() && (running("WinDivert") || running("WinDivert14")) {
        add(&mut found, "WinDivert");
    }
    found
}

/// zapret on Linux/macOS runs nfqws (NFQUEUE) or tpws (transparent proxy)
#[cfg(all(unix, not(target_os = "android")))]
pub fn detect() -> Vec<String> {
    let processes = std::process::Command::new("ps")
        .args(["-A", "-o", "comm="])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let running = processes
        .lines()
        .map(|l| l.trim().rsplit('/').next().unwrap_or(""))
        .any(|name| name == "nfqws" || name == "tpws");
    if running { vec!["zapret".to_string()] } else { Vec::new() }
}

#[cfg(target_os = "android")]
pub fn detect() -> Vec<String> {
    Vec::new()
}

