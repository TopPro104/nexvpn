use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub hwid: String,
    pub platform: String,
    pub os_version: String,
    pub model: String,
    pub user_agent: String,
}

// ── Windows ──────────────────────────────────────────

#[cfg(target_os = "windows")]
pub fn get_device_info() -> DeviceInfo {
    use winreg::enums::*;
    use winreg::RegKey;

    fn generate_hwid() -> String {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        hklm.open_subkey("SOFTWARE\\Microsoft\\Cryptography")
            .and_then(|key| key.get_value::<String, _>("MachineGuid"))
            .unwrap_or_default()
    }

    fn get_os_version() -> String {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm.open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
        match key {
            Ok(key) => {
                let major: u32 = key.get_value("CurrentMajorVersionNumber").unwrap_or(10);
                let minor: u32 = key.get_value("CurrentMinorVersionNumber").unwrap_or(0);
                let build: String = key.get_value("CurrentBuildNumber").unwrap_or_default();
                format!("{}.{}.{}", major, minor, build)
            }
            Err(_) => "10.0".to_string(),
        }
    }

    fn get_device_model() -> String {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm.open_subkey("SYSTEM\\CurrentControlSet\\Control\\SystemInformation");
        match key {
            Ok(key) => {
                let manufacturer: String = key.get_value("SystemManufacturer").unwrap_or_default();
                let product: String = key.get_value("SystemProductName").unwrap_or_default();
                let result = format!("{} {}", manufacturer, product).trim().to_string();
                if result.is_empty() { "PC-Desktop".to_string() } else { result }
            }
            Err(_) => "PC-Desktop".to_string(),
        }
    }

    DeviceInfo {
        hwid: generate_hwid(),
        platform: "Windows".to_string(),
        os_version: get_os_version(),
        model: get_device_model(),
        user_agent: "NexVPN/1.0".to_string(),
    }
}

// ── Android ──────────────────────────────────────────

#[cfg(target_os = "android")]
pub fn get_device_info() -> DeviceInfo {
    // On Android, use boot_id as a per-boot unique ID
    let hwid = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string())
        .trim()
        .to_string();

    // Read Android version from system properties
    let os_version = read_android_prop("ro.build.version.release")
        .unwrap_or_else(|| "Unknown".to_string());

    let manufacturer = read_android_prop("ro.product.manufacturer")
        .unwrap_or_else(|| "Android".to_string());
    let model_name = read_android_prop("ro.product.model")
        .unwrap_or_else(|| "Device".to_string());

    DeviceInfo {
        hwid,
        platform: "Android".to_string(),
        os_version,
        model: format!("{} {}", manufacturer, model_name),
        user_agent: "NexVPN/1.0".to_string(),
    }
}

#[cfg(target_os = "android")]
fn read_android_prop(name: &str) -> Option<String> {
    std::process::Command::new("getprop")
        .arg(name)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.is_empty() { None } else { Some(s) }
            } else {
                None
            }
        })
}

// ── Linux / macOS fallback ───────────────────────────

#[cfg(not(any(target_os = "windows", target_os = "android")))]
pub fn get_device_info() -> DeviceInfo {
    let hwid = std::fs::read_to_string("/etc/machine-id")
        .or_else(|_| std::fs::read_to_string("/var/lib/dbus/machine-id"))
        .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string())
        .trim()
        .to_string();

    DeviceInfo {
        hwid,
        platform: std::env::consts::OS.to_string(),
        os_version: "Unknown".to_string(),
        model: "Desktop".to_string(),
        user_agent: "NexVPN/1.0".to_string(),
    }
}
