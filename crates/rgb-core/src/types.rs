//! Plain data types shared by every driver and serialized to the frontend.

use serde::{Deserialize, Serialize};
use std::fmt;

/// 24-bit RGB colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Rgb = Rgb::new(0, 0, 0);
    pub const WHITE: Rgb = Rgb::new(255, 255, 255);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    Motherboard,
    Gpu,
    Dram,
    Keyboard,
    Mouse,
    Speaker,
    Headset,
    LedStrip,
    Light,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    Usb,
    Smbus,
    I2c,
    Ble,
    Virtual,
}

/// Vendor-neutral lighting effects. Drivers translate these into their own
/// protocol-specific mode numbers and only advertise the ones they support.
///
/// The numeric value is the `effect_id` passed to
/// [`RgbController::set_effect`](crate::RgbController::set_effect).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum EffectId {
    Off = 0,
    Static = 1,
    Breathing = 2,
    SpectrumCycle = 3,
    RainbowWave = 4,
    ColorPulse = 5,
    Flashing = 6,
}

impl EffectId {
    pub const ALL: [EffectId; 7] = [
        EffectId::Off,
        EffectId::Static,
        EffectId::Breathing,
        EffectId::SpectrumCycle,
        EffectId::RainbowWave,
        EffectId::ColorPulse,
        EffectId::Flashing,
    ];

    pub fn name(self) -> &'static str {
        match self {
            EffectId::Off => "Off",
            EffectId::Static => "Static",
            EffectId::Breathing => "Breathing",
            EffectId::SpectrumCycle => "Spectrum Cycle",
            EffectId::RainbowWave => "Rainbow Wave",
            EffectId::ColorPulse => "Color Pulse",
            EffectId::Flashing => "Flashing",
        }
    }

    /// Whether the effect renders the user-selected colour.
    pub fn uses_color(self) -> bool {
        matches!(
            self,
            EffectId::Static | EffectId::Breathing | EffectId::ColorPulse | EffectId::Flashing
        )
    }

    /// Whether the effect is animated (and therefore honours `speed`).
    pub fn uses_speed(self) -> bool {
        !matches!(self, EffectId::Off | EffectId::Static)
    }

    pub fn info(self) -> EffectInfo {
        EffectInfo {
            id: self as u8,
            name: self.name().to_string(),
            uses_color: self.uses_color(),
            uses_speed: self.uses_speed(),
        }
    }
}

impl TryFrom<u8> for EffectId {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        EffectId::ALL
            .into_iter()
            .find(|e| *e as u8 == value)
            .ok_or_else(|| format!("unknown effect id {value}"))
    }
}

/// Speed is expressed as a percentage: `0` = slowest, `100` = fastest.
/// Values above 100 are clamped.
pub const SPEED_MAX: u8 = 100;

/// Linear map of a 0–100 speed onto an inclusive `[slow, fast]` range of any
/// integer-like scale (which may be inverted, e.g. periods in ms).
pub fn scale_speed(speed: u8, slow: i64, fast: i64) -> i64 {
    let s = i64::from(speed.min(SPEED_MAX));
    slow + (fast - slow) * s / i64::from(SPEED_MAX)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectInfo {
    pub id: u8,
    pub name: String,
    pub uses_color: bool,
    pub uses_speed: bool,
}

/// Battery state of a wireless device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryInfo {
    /// 0–100.
    pub percent: u8,
    pub charging: bool,
    /// Estimated from voltage rather than reported by the device.
    pub approximate: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneInfo {
    pub index: u8,
    pub name: String,
    pub led_count: u16,
}

/// Static description of a controllable device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Stable identifier, unique across all drivers (e.g.
    /// `logitech:046d:c339:<serial>:ff`). Used as the IPC handle.
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub device_type: DeviceType,
    pub transport: Transport,
    /// Human-readable bus location (HID path, `i2c-3 @ 0x4e`, BLE address…).
    pub location: String,
    pub serial: Option<String>,
    pub firmware: Option<String>,
    pub zones: Vec<ZoneInfo>,
    pub effects: Vec<EffectInfo>,
    /// Physical key layout when the device supports per-key colours
    /// (e.g. `"logitech-g910"`); `None` for zone-only devices.
    #[serde(default)]
    pub layout: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_round_trip() {
        for e in EffectId::ALL {
            assert_eq!(EffectId::try_from(e as u8).unwrap(), e);
        }
        assert!(EffectId::try_from(200).is_err());
    }

    #[test]
    fn speed_scaling() {
        assert_eq!(scale_speed(0, 20_000, 1_000), 20_000);
        assert_eq!(scale_speed(100, 20_000, 1_000), 1_000);
        assert_eq!(scale_speed(255, 20_000, 1_000), 1_000);
        assert_eq!(scale_speed(50, 0, 4), 2);
    }

    #[test]
    fn rgb_display() {
        assert_eq!(Rgb::new(255, 0, 16).to_string(), "#ff0010");
    }
}
