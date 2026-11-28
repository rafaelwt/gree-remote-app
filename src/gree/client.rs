use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};

use super::crypto::{
    EncryptedPack, EncryptionMode, GENERIC_ECB_KEY, GENERIC_GCM_KEY, decrypt_pack, encrypt_pack,
};
use super::protocol::{
    AcStatus, BindResponsePack, CmdResponsePack, DeviceInfo, Envelope, GREE_PORT, STATUS_COLS,
    ScanPack, StatusResponsePack,
};

fn build_request_envelope(tcid: &str, enc: &EncryptedPack, i_val: u8) -> String {
    match &enc.tag {
        Some(tag) => format!(
            r#"{{"cid":"app","i":{},"t":"pack","uid":0,"tcid":"{}","tag":"{}","pack":"{}"}}"#,
            i_val, tcid, tag, enc.pack
        ),
        None => format!(
            r#"{{"cid":"app","i":{},"t":"pack","uid":0,"tcid":"{}","pack":"{}"}}"#,
            i_val, tcid, enc.pack
        ),
    }
}

fn parse_envelope(data: &[u8]) -> Result<Envelope, String> {
    let slice = match data.iter().rposition(|&b| b == b'}') {
        Some(pos) => &data[..=pos],
        None => data,
    };
    serde_json::from_slice::<Envelope>(slice)
        .map_err(|e| format!("Invalid JSON envelope: {e}"))
}

fn send_unicast(ip: Ipv4Addr, payload: &[u8], timeout: Duration) -> Result<Vec<u8>, String> {
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("UDP bind error: {e}"))?;
    sock.set_read_timeout(Some(timeout))
        .map_err(|e| format!("UDP timeout error: {e}"))?;
    sock.send_to(payload, SocketAddrV4::new(ip, GREE_PORT))
        .map_err(|e| format!("UDP send error to {ip}: {e}"))?;

    let mut buf = [0u8; 4096];
    let (n, _) = sock
        .recv_from(&mut buf)
        .map_err(|e| format!("No response from {ip}: {e}"))?;
    Ok(buf[..n].to_vec())
}

fn parse_scan_response(ip: Ipv4Addr, data: &[u8]) -> Result<DeviceInfo, String> {
    let env = parse_envelope(data)?;
    let pack_b64 = env
        .pack
        .as_deref()
        .ok_or_else(|| "Scan response missing 'pack'".to_string())?;

    let encryption = if env.tag.is_some() {
        EncryptionMode::Gcm
    } else {
        EncryptionMode::Ecb
    };
    let generic_key = match encryption {
        EncryptionMode::Ecb => GENERIC_ECB_KEY,
        EncryptionMode::Gcm => GENERIC_GCM_KEY,
    };

    let decrypted = decrypt_pack(pack_b64, env.tag.as_deref(), generic_key, encryption)?;
    let scan: ScanPack =
        serde_json::from_str(&decrypted).map_err(|e| format!("Invalid scan pack JSON: {e}"))?;

    let id = if !scan.cid.is_empty() {
        scan.cid
    } else if !env.cid.is_empty() {
        env.cid
    } else if !scan.mac.is_empty() {
        scan.mac
    } else {
        return Err("Device did not report MAC/CID".to_string());
    };

    Ok(DeviceInfo {
        ip,
        id,
        name: scan.name,
        brand: scan.brand,
        model: scan.model,
        model_type: scan.model_type,
        ver: scan.ver,
        hid: scan.hid,
        encryption,
        key: None,
    })
}

/// Collects local non-virtual IPv4 interfaces (excluding loopback, docker/bridge/virbr/tailscale)
/// to scan via both broadcast and unicast /24 sweep.
fn detect_lan_subnets() -> Vec<(Ipv4Addr, Option<Ipv4Addr>)> {
    let mut result = Vec::new();
    let Ok(ifaces) = if_addrs::get_if_addrs() else {
        return result;
    };

    for iface in ifaces {
        if iface.is_loopback() {
            continue;
        }
        let name = iface.name.as_str();
        if name.starts_with("docker")
            || name.starts_with("br-")
            || name.starts_with("virbr")
            || name.starts_with("veth")
            || name.starts_with("tailscale")
            || name.starts_with("wg")
        {
            continue;
        }
        if let if_addrs::IfAddr::V4(v4) = iface.addr {
            result.push((v4.ip, v4.broadcast));
        }
    }
    result
}

/// Scans the local network using both UDP broadcast and unicast /24 sweep,
/// then automatically binds to each discovered device.
pub fn scan_and_bind(manual_ip: Option<Ipv4Addr>) -> Result<Vec<DeviceInfo>, String> {
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("UDP bind error: {e}"))?;
    sock.set_broadcast(true)
        .map_err(|e| format!("UDP broadcast error: {e}"))?;

    let scan_msg = br#"{"t":"scan"}"#;

    if let Some(ip) = manual_ip {
        let _ = sock.send_to(scan_msg, SocketAddrV4::new(ip, GREE_PORT));
    } else {
        let subnets = detect_lan_subnets();
        let _ = sock.send_to(
            scan_msg,
            SocketAddrV4::new(Ipv4Addr::BROADCAST, GREE_PORT),
        );

        let mut swept_prefixes = HashSet::new();
        for (local_ip, bcast) in subnets {
            if let Some(b) = bcast {
                let _ = sock.send_to(scan_msg, SocketAddrV4::new(b, GREE_PORT));
            }
            let octets = local_ip.octets();
            let prefix = (octets[0], octets[1], octets[2]);
            if swept_prefixes.insert(prefix) {
                for host in 1u8..=254u8 {
                    if host == octets[3] {
                        continue;
                    }
                    let target = Ipv4Addr::new(prefix.0, prefix.1, prefix.2, host);
                    let _ = sock.send_to(scan_msg, SocketAddrV4::new(target, GREE_PORT));
                }
            }
        }
    }

    let mut devices = Vec::new();
    let mut seen_ids = HashSet::new();
    let deadline = Instant::now() + Duration::from_millis(2500);
    let mut buf = [0u8; 4096];

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let _ = sock.set_read_timeout(Some(remaining.min(Duration::from_millis(300))));
        match sock.recv_from(&mut buf) {
            Ok((n, src)) => {
                if let IpAddr::V4(ip) = src.ip()
                    && let Ok(dev) = parse_scan_response(ip, &buf[..n])
                    && seen_ids.insert(dev.id.clone())
                {
                    devices.push(dev);
                }
            }
            Err(_) => continue,
        }
    }

    for dev in &mut devices {
        if let Err(e) = bind_device(dev) {
            eprintln!("Bind warning for {}: {}", dev.ip, e);
        }
    }

    Ok(devices)
}

/// Binds to a Gree device to obtain its unique AES key.
/// Tries the device's detected encryption mode first; if ECB times out, falls back to GCM.
pub fn bind_device(dev: &mut DeviceInfo) -> Result<String, String> {
    match try_bind(dev.ip, &dev.id, dev.encryption) {
        Ok(key) => {
            dev.key = Some(key.clone());
            Ok(key)
        }
        Err(first_err) if dev.encryption == EncryptionMode::Ecb => {
            match try_bind(dev.ip, &dev.id, EncryptionMode::Gcm) {
                Ok(key) => {
                    dev.encryption = EncryptionMode::Gcm;
                    dev.key = Some(key.clone());
                    Ok(key)
                }
                Err(second_err) => Err(format!(
                    "Bind failed (ECB: {first_err}; GCM: {second_err})"
                )),
            }
        }
        Err(e) => Err(e),
    }
}

fn try_bind(ip: Ipv4Addr, id: &str, mode: EncryptionMode) -> Result<String, String> {
    let pack_json = format!(r#"{{"mac":"{}","t":"bind","uid":0}}"#, id);
    let generic_key = match mode {
        EncryptionMode::Ecb => GENERIC_ECB_KEY,
        EncryptionMode::Gcm => GENERIC_GCM_KEY,
    };
    let enc = encrypt_pack(&pack_json, generic_key, mode)?;
    let req = build_request_envelope(id, &enc, 1);

    let resp_bytes = send_unicast(ip, req.as_bytes(), Duration::from_secs(3))?;
    let env = parse_envelope(&resp_bytes)?;
    let pack_b64 = env
        .pack
        .as_deref()
        .ok_or_else(|| "Bind response missing 'pack'".to_string())?;

    let decrypted = decrypt_pack(pack_b64, env.tag.as_deref(), generic_key, mode)?;
    let bind_resp: BindResponsePack =
        serde_json::from_str(&decrypted).map_err(|e| format!("Invalid bind pack JSON: {e}"))?;

    if bind_resp.t.eq_ignore_ascii_case("bindok") && !bind_resp.key.is_empty() {
        Ok(bind_resp.key)
    } else {
        Err(format!("Bind rejected: t={}, r={}", bind_resp.t, bind_resp.r))
    }
}

pub fn get_status(dev: &DeviceInfo) -> Result<AcStatus, String> {
    let key = dev
        .key
        .as_deref()
        .ok_or_else(|| "Device is not bound (missing key)".to_string())?;

    let cols_json = serde_json::to_string(STATUS_COLS).unwrap();
    let pack_json = format!(
        r#"{{"cols":{},"mac":"{}","t":"status"}}"#,
        cols_json, dev.id
    );
    let enc = encrypt_pack(&pack_json, key, dev.encryption)?;
    let req = build_request_envelope(&dev.id, &enc, 0);

    let resp_bytes = send_unicast(dev.ip, req.as_bytes(), Duration::from_secs(3))?;
    let env = parse_envelope(&resp_bytes)?;
    let pack_b64 = env
        .pack
        .as_deref()
        .ok_or_else(|| "Status response missing 'pack'".to_string())?;

    let decrypted = decrypt_pack(pack_b64, env.tag.as_deref(), key, dev.encryption)?;
    let status_resp: StatusResponsePack =
        serde_json::from_str(&decrypted).map_err(|e| format!("Invalid status pack JSON: {e}"))?;

    Ok(AcStatus::from_response(&status_resp))
}

pub fn set_params(dev: &DeviceInfo, params: &[(&str, i64)]) -> Result<(), String> {
    let key = dev
        .key
        .as_deref()
        .ok_or_else(|| "Device is not bound (missing key)".to_string())?;

    let opts: Vec<&str> = params.iter().map(|(k, _)| *k).collect();
    let vals: Vec<i64> = params.iter().map(|(_, v)| *v).collect();
    let pack_json = format!(
        r#"{{"opt":{},"p":{},"t":"cmd"}}"#,
        serde_json::to_string(&opts).unwrap(),
        serde_json::to_string(&vals).unwrap()
    );

    let enc = encrypt_pack(&pack_json, key, dev.encryption)?;
    let req = build_request_envelope(&dev.id, &enc, 0);

    let resp_bytes = send_unicast(dev.ip, req.as_bytes(), Duration::from_secs(3))?;
    let env = parse_envelope(&resp_bytes)?;
    let pack_b64 = env
        .pack
        .as_deref()
        .ok_or_else(|| "Command response missing 'pack'".to_string())?;

    let decrypted = decrypt_pack(pack_b64, env.tag.as_deref(), key, dev.encryption)?;
    let cmd_resp: CmdResponsePack =
        serde_json::from_str(&decrypted).map_err(|e| format!("Invalid cmd response JSON: {e}"))?;

    if cmd_resp.r == 200 {
        Ok(())
    } else {
        Err(format!("Device returned error code r={}", cmd_resp.r))
    }
}

/// Synchronizes the AC's internal clock with the host's local time.
pub fn sync_device_time(dev: &DeviceInfo) -> Result<(), String> {
    let key = dev
        .key
        .as_deref()
        .ok_or_else(|| "Device is not bound (missing key)".to_string())?;

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let pack_json = format!(
        r#"{{"opt":["time"],"p":["{}"],"sub":"{}","t":"cmd"}}"#,
        now, dev.id
    );
    let enc = encrypt_pack(&pack_json, key, dev.encryption)?;
    let req = build_request_envelope(&dev.id, &enc, 0);

    let resp_bytes = send_unicast(dev.ip, req.as_bytes(), Duration::from_secs(3))?;
    let env = parse_envelope(&resp_bytes)?;
    let pack_b64 = env
        .pack
        .as_deref()
        .ok_or_else(|| "Time sync response missing 'pack'".to_string())?;

    let decrypted = decrypt_pack(pack_b64, env.tag.as_deref(), key, dev.encryption)?;
    let cmd_resp: CmdResponsePack =
        serde_json::from_str(&decrypted).map_err(|e| format!("Invalid time sync JSON: {e}"))?;

    if cmd_resp.r == 200 {
        Ok(())
    } else {
        Err(format!("Time sync rejected with r={}", cmd_resp.r))
    }
}

/// Sends a native Gree schedule (`setT`) to turn the AC on or off at `(hour, minute)`.
/// First syncs the device clock so the AC triggers at the exact local time.
pub fn set_device_timer(
    dev: &DeviceInfo,
    enable: bool,
    power_on: bool,
    hour: u32,
    minute: u32,
    weekday_sun0: usize,
) -> Result<(), String> {
    let key = dev
        .key
        .as_deref()
        .ok_or_else(|| "Device is not bound (missing key)".to_string())?;

    if enable {
        let _ = sync_device_time(dev);
    }

    let mut week = [0u8; 7];
    if weekday_sun0 < 7 {
        week[weekday_sun0] = 1;
    }
    let tz_hours = chrono::Local::now().offset().local_minus_utc() / 3600;
    let pack_json = serde_json::json!({
        "cmd": [{
            "mac": [dev.id],
            "opt": ["Pow"],
            "p": [i64::from(power_on)]
        }],
        "enable": u8::from(enable),
        "hr": hour,
        "id": 0,
        "min": minute,
        "name": "4772656554696d6572",
        "sec": 0,
        "t": "setT",
        "tz": tz_hours,
        "week": week
    })
    .to_string();

    let enc = encrypt_pack(&pack_json, key, dev.encryption)?;
    let req = build_request_envelope(&dev.id, &enc, 0);

    let resp_bytes = send_unicast(dev.ip, req.as_bytes(), Duration::from_secs(3))?;
    let env = parse_envelope(&resp_bytes)?;
    let pack_b64 = env
        .pack
        .as_deref()
        .ok_or_else(|| "Timer response missing 'pack'".to_string())?;

    let decrypted = decrypt_pack(pack_b64, env.tag.as_deref(), key, dev.encryption)?;
    let cmd_resp: CmdResponsePack =
        serde_json::from_str(&decrypted).map_err(|e| format!("Invalid timer response JSON: {e}"))?;

    if cmd_resp.r == 200 {
        Ok(())
    } else {
        Err(format!("Device rejected timer with r={}", cmd_resp.r))
    }
}
