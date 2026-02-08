use serde::Serialize;
use winreg::enums::*;
use winreg::RegKey;

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub hwid: String,
    pub platform: String,
    pub os_version: String,
    pub model: String,
    pub user_agent: String,
}

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

pub fn get_device_info() -> DeviceInfo {
    DeviceInfo {
        hwid: generate_hwid(),
        platform: "Windows".to_string(),
        os_version: get_os_version(),
        model: get_device_model(),
        user_agent: "NexVPN/0.1".to_string(),
    }
}
