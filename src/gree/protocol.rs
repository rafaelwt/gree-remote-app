use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

use super::crypto::EncryptionMode;

pub const GREE_PORT: u16 = 7000;

pub const STATUS_COLS: &[&str] = &[
    "Pow",
    "Mod",
    "SetTem",
    "TemSen",
    "WdSpd",
    "Air",
    "Blo",
    "Health",
    "SwhSlp",
    "Lig",
    "SwingLfRig",
    "SwUpDn",
    "Quiet",
    "Tur",
    "StHt",
    "TemUn",
    "TemRec",
    "SvSt",
];

#[derive(Debug, Clone, Deserialize)]
pub struct Envelope {
    #[serde(default)]
    pub cid: String,
    #[serde(default)]
    pub pack: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScanPack {
    #[serde(default)]
    pub cid: String,
    #[serde(default)]
    pub mac: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub brand: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub ver: String,
    #[serde(default, rename = "ModelType")]
    pub model_type: String,
    #[serde(default)]
    pub hid: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BindResponsePack {
    #[serde(default)]
    pub t: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub r: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatusResponsePack {
    #[serde(default)]
    pub cols: Vec<String>,
    #[serde(default)]
    pub dat: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CmdResponsePack {
    #[serde(default)]
    pub r: i32,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DeviceInfo {
    pub ip: Ipv4Addr,
    pub id: String,
    pub name: String,
    pub brand: String,
    pub model: String,
    pub model_type: String,
    pub ver: String,
    pub hid: String,
    pub encryption: EncryptionMode,
    pub key: Option<String>,
}

impl DeviceInfo {
    pub fn display_name(&self) -> String {
        if !self.name.trim().is_empty() {
            format!("{} ({})", self.name, self.ip)
        } else if !self.brand.trim().is_empty() {
            format!("{} {} ({})", self.brand.to_uppercase(), self.model_type, self.ip)
        } else {
            format!("Gree AC ({})", self.ip)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcMode {
    Auto = 0,
    Cool = 1,
    Dry = 2,
    Fan = 3,
    Heat = 4,
}

#[allow(dead_code)]
impl AcMode {
    pub const ALL: [AcMode; 5] = [
        AcMode::Auto,
        AcMode::Cool,
        AcMode::Dry,
        AcMode::Fan,
        AcMode::Heat,
    ];

    pub fn from_i64(v: i64) -> Self {
        match v {
            1 => AcMode::Cool,
            2 => AcMode::Dry,
            3 => AcMode::Fan,
            4 => AcMode::Heat,
            _ => AcMode::Auto,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AcMode::Auto => "Auto",
            AcMode::Cool => "Frío (Cool)",
            AcMode::Dry => "Seco (Dry)",
            AcMode::Fan => "Ventilador (Fan)",
            AcMode::Heat => "Calor (Heat)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanSpeed {
    Auto = 0,
    Low = 1,
    MediumLow = 2,
    Medium = 3,
    MediumHigh = 4,
    High = 5,
}

#[allow(dead_code)]
impl FanSpeed {
    pub const ALL: [FanSpeed; 6] = [
        FanSpeed::Auto,
        FanSpeed::Low,
        FanSpeed::MediumLow,
        FanSpeed::Medium,
        FanSpeed::MediumHigh,
        FanSpeed::High,
    ];

    pub fn from_i64(v: i64) -> Self {
        match v {
            1 => FanSpeed::Low,
            2 => FanSpeed::MediumLow,
            3 => FanSpeed::Medium,
            4 => FanSpeed::MediumHigh,
            5 => FanSpeed::High,
            _ => FanSpeed::Auto,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FanSpeed::Auto => "Auto",
            FanSpeed::Low => "Baja (1)",
            FanSpeed::MediumLow => "Media-Baja (2)",
            FanSpeed::Medium => "Media (3)",
            FanSpeed::MediumHigh => "Media-Alta (4)",
            FanSpeed::High => "Alta (5)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalSwing {
    Default = 0,
    FullSwing = 1,
    FixedTop = 2,
    FixedMidTop = 3,
    FixedMid = 4,
    FixedMidLow = 5,
    FixedLow = 6,
    SwingLow = 7,
    SwingMidLow = 8,
    SwingMid = 9,
    SwingMidTop = 10,
    SwingTop = 11,
}

impl VerticalSwing {
    pub const ALL: [VerticalSwing; 7] = [
        VerticalSwing::Default,
        VerticalSwing::FullSwing,
        VerticalSwing::FixedTop,
        VerticalSwing::FixedMidTop,
        VerticalSwing::FixedMid,
        VerticalSwing::FixedMidLow,
        VerticalSwing::FixedLow,
    ];

    pub fn from_i64(v: i64) -> Self {
        match v {
            1 => VerticalSwing::FullSwing,
            2 => VerticalSwing::FixedTop,
            3 => VerticalSwing::FixedMidTop,
            4 => VerticalSwing::FixedMid,
            5 => VerticalSwing::FixedMidLow,
            6 => VerticalSwing::FixedLow,
            7 => VerticalSwing::SwingLow,
            8 => VerticalSwing::SwingMidLow,
            9 => VerticalSwing::SwingMid,
            10 => VerticalSwing::SwingMidTop,
            11 => VerticalSwing::SwingTop,
            _ => VerticalSwing::Default,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            VerticalSwing::Default => "Apagado / Default",
            VerticalSwing::FullSwing => "Oscilación completa",
            VerticalSwing::FixedTop => "Fijo arriba (1/5)",
            VerticalSwing::FixedMidTop => "Fijo medio-arriba (2/5)",
            VerticalSwing::FixedMid => "Fijo centro (3/5)",
            VerticalSwing::FixedMidLow => "Fijo medio-abajo (4/5)",
            VerticalSwing::FixedLow => "Fijo abajo (5/5)",
            VerticalSwing::SwingLow => "Oscilar abajo",
            VerticalSwing::SwingMidLow => "Oscilar medio-abajo",
            VerticalSwing::SwingMid => "Oscilar centro",
            VerticalSwing::SwingMidTop => "Oscilar medio-arriba",
            VerticalSwing::SwingTop => "Oscilar arriba",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalSwing {
    Default = 0,
    FullSwing = 1,
    FixedLeft = 2,
    FixedMidLeft = 3,
    FixedMid = 4,
    FixedMidRight = 5,
    FixedRight = 6,
}

impl HorizontalSwing {
    pub const ALL: [HorizontalSwing; 7] = [
        HorizontalSwing::Default,
        HorizontalSwing::FullSwing,
        HorizontalSwing::FixedLeft,
        HorizontalSwing::FixedMidLeft,
        HorizontalSwing::FixedMid,
        HorizontalSwing::FixedMidRight,
        HorizontalSwing::FixedRight,
    ];

    pub fn from_i64(v: i64) -> Self {
        match v {
            1 => HorizontalSwing::FullSwing,
            2 => HorizontalSwing::FixedLeft,
            3 => HorizontalSwing::FixedMidLeft,
            4 => HorizontalSwing::FixedMid,
            5 => HorizontalSwing::FixedMidRight,
            6 => HorizontalSwing::FixedRight,
            _ => HorizontalSwing::Default,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HorizontalSwing::Default => "Apagado / Default",
            HorizontalSwing::FullSwing => "Oscilación completa",
            HorizontalSwing::FixedLeft => "Izquierda",
            HorizontalSwing::FixedMidLeft => "Centro-Izquierda",
            HorizontalSwing::FixedMid => "Centro",
            HorizontalSwing::FixedMidRight => "Centro-Derecha",
            HorizontalSwing::FixedRight => "Derecha",
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AcStatus {
    pub power: bool,
    pub mode: AcMode,
    pub set_temp: i64,
    pub current_temp: Option<i64>,
    pub fan_speed: FanSpeed,
    pub vertical_swing: VerticalSwing,
    pub horizontal_swing: HorizontalSwing,
    pub light: bool,
    pub turbo: bool,
    pub quiet: bool,
    pub sleep: bool,
    pub xfan: bool,
    pub health: bool,
    pub air: bool,
    pub saving: bool,
    pub freeze_guard: bool,
    pub fahrenheit: bool,
}

impl Default for AcStatus {
    fn default() -> Self {
        Self {
            power: false,
            mode: AcMode::Cool,
            set_temp: 24,
            current_temp: None,
            fan_speed: FanSpeed::Auto,
            vertical_swing: VerticalSwing::Default,
            horizontal_swing: HorizontalSwing::Default,
            light: true,
            turbo: false,
            quiet: false,
            sleep: false,
            xfan: false,
            health: false,
            air: false,
            saving: false,
            freeze_guard: false,
            fahrenheit: false,
        }
    }
}

impl AcStatus {
    pub fn from_response(resp: &StatusResponsePack) -> Self {
        let mut map: HashMap<&str, i64> = HashMap::new();
        for (col, val) in resp.cols.iter().zip(resp.dat.iter()) {
            if let Some(n) = val.as_i64() {
                map.insert(col.as_str(), n);
            }
        }
        let get = |k: &str| map.get(k).copied().unwrap_or(0);
        let current_temp = map.get("TemSen").copied().and_then(|v| {
            // Gree reports internal temperature with +40 offset (0 means sensor absent)
            if v > 0 { Some(v - 40) } else { None }
        });

        Self {
            power: get("Pow") != 0,
            mode: AcMode::from_i64(get("Mod")),
            set_temp: map.get("SetTem").copied().unwrap_or(24),
            current_temp,
            fan_speed: FanSpeed::from_i64(get("WdSpd")),
            vertical_swing: VerticalSwing::from_i64(get("SwUpDn")),
            horizontal_swing: HorizontalSwing::from_i64(get("SwingLfRig")),
            light: get("Lig") != 0,
            turbo: get("Tur") != 0,
            quiet: get("Quiet") != 0,
            sleep: get("SwhSlp") != 0,
            xfan: get("Blo") != 0,
            health: get("Health") != 0,
            air: get("Air") != 0,
            saving: get("SvSt") != 0,
            freeze_guard: get("StHt") != 0,
            fahrenheit: get("TemUn") != 0,
        }
    }
}
