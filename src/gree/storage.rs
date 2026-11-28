use std::fs;
use std::path::PathBuf;

use super::protocol::DeviceInfo;

fn config_file_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from)?;

    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)?
        .join("Library")
        .join("Application Support");

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|h| h.join(".config"))
        })?;

    Some(base.join("gree-remote-app").join("devices.json"))
}

pub fn load_devices() -> Vec<DeviceInfo> {
    let Some(path) = config_file_path() else {
        return Vec::new();
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<DeviceInfo>>(&raw).unwrap_or_default()
}

pub fn save_devices(devices: &[DeviceInfo]) -> Result<(), String> {
    if devices.is_empty() {
        return Ok(());
    }
    let path = config_file_path().ok_or_else(|| "Could not determine config directory".to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed creating config dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(devices)
        .map_err(|e| format!("Failed serializing devices: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("Failed writing {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gree::crypto::EncryptionMode;
    use std::net::Ipv4Addr;

    #[test]
    fn devices_json_roundtrip() {
        let list = vec![DeviceInfo {
            ip: Ipv4Addr::new(192, 168, 1, 100),
            id: "001122334455".to_string(),
            name: "Living Room".to_string(),
            brand: "gree".to_string(),
            model: "gree".to_string(),
            model_type: "32792".to_string(),
            ver: "V2.0.0".to_string(),
            hid: "test-hid".to_string(),
            encryption: EncryptionMode::Gcm,
            key: Some("0123456789abcdef".to_string()),
        }];
        let json = serde_json::to_string(&list).unwrap();
        let parsed: Vec<DeviceInfo> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].ip, Ipv4Addr::new(192, 168, 1, 100));
        assert_eq!(parsed[0].encryption, EncryptionMode::Gcm);
        assert_eq!(parsed[0].key.as_deref(), Some("0123456789abcdef"));
    }
}
