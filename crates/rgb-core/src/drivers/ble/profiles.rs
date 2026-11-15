//! GATT protocols spoken by BLE LED controllers. Pure encoders, unit-tested.
//!
//! * **RGBVE** – this project's open protocol for DIY ESP32 controllers
//!   (firmware in `firmware/esp32-ble-rgb`). Supports every unified effect.
//! * **ELK-BLEDOM** – the ubiquitous cheap LED-strip controller
//!   (service `FFF0`, characteristic `FFF3`, `7E … EF` frames).
//! * **Triones / HappyLighting** – service `FFD5`, characteristic `FFD9`.

use btleplug::api::bleuuid::uuid_from_u16;
use uuid::Uuid;

use crate::types::{scale_speed, EffectId, Rgb};

pub const RGBVE_SERVICE: Uuid = Uuid::from_u128(0x8d2c0001_8b5e_4f3a_9c1d_5e6f7a8b9c0d);
pub const RGBVE_CONTROL_CHAR: Uuid = Uuid::from_u128(0x8d2c0002_8b5e_4f3a_9c1d_5e6f7a8b9c0d);

pub mod rgbve_op {
    pub const SET_COLOR: u8 = 0x01;
    pub const SET_EFFECT: u8 = 0x02;
    pub const POWER: u8 = 0x03;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BleProfile {
    Rgbve,
    ElkBledom,
    Triones,
}

impl BleProfile {
    pub fn label(self) -> &'static str {
        match self {
            Self::Rgbve => "RGBVE ESP32",
            Self::ElkBledom => "ELK-BLEDOM",
            Self::Triones => "Triones",
        }
    }

    pub fn service(self) -> Uuid {
        match self {
            Self::Rgbve => RGBVE_SERVICE,
            Self::ElkBledom => uuid_from_u16(0xFFF0),
            Self::Triones => uuid_from_u16(0xFFD5),
        }
    }

    pub fn characteristic(self) -> Uuid {
        match self {
            Self::Rgbve => RGBVE_CONTROL_CHAR,
            Self::ElkBledom => uuid_from_u16(0xFFF3),
            Self::Triones => uuid_from_u16(0xFFD9),
        }
    }

    /// Identify a peripheral from its advertisement. Service UUIDs win;
    /// names are a fallback because many cheap controllers advertise none.
    pub fn identify(name: Option<&str>, services: &[Uuid]) -> Option<Self> {
        for p in [Self::Rgbve, Self::ElkBledom, Self::Triones] {
            if services.contains(&p.service()) {
                return Some(p);
            }
        }
        let name = name?.to_ascii_uppercase();
        if name.starts_with("RGBVE") {
            Some(Self::Rgbve)
        } else if ["ELK-", "MELK", "LEDBLE"]
            .iter()
            .any(|p| name.starts_with(p))
        {
            Some(Self::ElkBledom)
        } else if ["TRIONES", "LEDBLUE", "QHM-", "DREAM~"]
            .iter()
            .any(|p| name.starts_with(p))
        {
            Some(Self::Triones)
        } else {
            None
        }
    }

    pub fn supported_effects(self) -> &'static [EffectId] {
        use EffectId::*;
        match self {
            Self::Rgbve => &EffectId::ALL,
            Self::ElkBledom | Self::Triones => &[Off, Static, SpectrumCycle],
        }
    }

    pub fn power(self, on: bool) -> Vec<u8> {
        match (self, on) {
            (Self::Rgbve, on) => vec![rgbve_op::POWER, on as u8],
            (Self::ElkBledom, true) => vec![0x7E, 0x00, 0x04, 0xF0, 0x00, 0x01, 0xFF, 0x00, 0xEF],
            (Self::ElkBledom, false) => vec![0x7E, 0x00, 0x04, 0x00, 0x00, 0x00, 0xFF, 0x00, 0xEF],
            (Self::Triones, true) => vec![0xCC, 0x23, 0x33],
            (Self::Triones, false) => vec![0xCC, 0x24, 0x33],
        }
    }

    pub fn color(self, c: Rgb) -> Vec<u8> {
        match self {
            Self::Rgbve => vec![rgbve_op::SET_COLOR, c.r, c.g, c.b],
            Self::ElkBledom => vec![0x7E, 0x00, 0x05, 0x03, c.r, c.g, c.b, 0x00, 0xEF],
            Self::Triones => vec![0x56, c.r, c.g, c.b, 0x00, 0xF0, 0xAA],
        }
    }

    /// Frames for an animated effect. `None` if the profile can't do it.
    /// (Off and Static are handled by the controller via `power`/`color`.)
    pub fn effect(self, effect: EffectId, speed: u8, color: Rgb) -> Option<Vec<Vec<u8>>> {
        match self {
            Self::Rgbve => Some(vec![vec![
                rgbve_op::SET_EFFECT,
                effect as u8,
                speed.min(100),
                color.r,
                color.g,
                color.b,
            ]]),
            Self::ElkBledom => match effect {
                // 0x8A = seven-colour cross-fade; speed register is 0–100.
                EffectId::SpectrumCycle => Some(vec![
                    vec![0x7E, 0x00, 0x03, 0x8A, 0x03, 0x00, 0x00, 0x00, 0xEF],
                    vec![
                        0x7E,
                        0x00,
                        0x02,
                        speed.min(100),
                        0x00,
                        0x00,
                        0x00,
                        0x00,
                        0xEF,
                    ],
                ]),
                _ => None,
            },
            Self::Triones => match effect {
                // 0x25 = seven-colour cross-fade; delay 0x1F (slow) … 0x01 (fast).
                EffectId::SpectrumCycle => {
                    let delay = scale_speed(speed, 0x1F, 0x01) as u8;
                    Some(vec![vec![0xBB, 0x25, delay, 0x44]])
                }
                _ => None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_by_service_then_name() {
        assert_eq!(
            BleProfile::identify(None, &[uuid_from_u16(0xFFD5)]),
            Some(BleProfile::Triones)
        );
        assert_eq!(
            BleProfile::identify(Some("ELK-BLEDOM   "), &[]),
            Some(BleProfile::ElkBledom)
        );
        assert_eq!(
            BleProfile::identify(Some("RGBVE Desk"), &[]),
            Some(BleProfile::Rgbve)
        );
        assert_eq!(BleProfile::identify(Some("Fitness Band"), &[]), None);
        assert_eq!(BleProfile::identify(None, &[]), None);
    }

    #[test]
    fn color_frames() {
        let c = Rgb::new(0x11, 0x22, 0x33);
        assert_eq!(
            BleProfile::ElkBledom.color(c),
            vec![0x7E, 0, 5, 3, 0x11, 0x22, 0x33, 0, 0xEF]
        );
        assert_eq!(
            BleProfile::Triones.color(c),
            vec![0x56, 0x11, 0x22, 0x33, 0, 0xF0, 0xAA]
        );
        assert_eq!(BleProfile::Rgbve.color(c), vec![1, 0x11, 0x22, 0x33]);
    }

    #[test]
    fn effect_support_matches_encoders() {
        for p in [
            BleProfile::Rgbve,
            BleProfile::ElkBledom,
            BleProfile::Triones,
        ] {
            for e in p.supported_effects() {
                if !matches!(e, EffectId::Off | EffectId::Static) {
                    assert!(p.effect(*e, 50, Rgb::WHITE).is_some(), "{p:?} {e:?}");
                }
            }
        }
        assert!(BleProfile::Triones
            .effect(EffectId::Breathing, 50, Rgb::WHITE)
            .is_none());
        assert_eq!(
            BleProfile::Triones
                .effect(EffectId::SpectrumCycle, 100, Rgb::WHITE)
                .unwrap()[0][2],
            0x01
        );
    }
}
