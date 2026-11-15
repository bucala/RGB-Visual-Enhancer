//! MSI GeForce "v2" RGB controller (ITE9) at I2C address 0x68, as used by
//! RTX 40-series GAMING X cards. Register map and write sequences as
//! documented by OpenRGB's `MSIGPUv2Controller`. Pure: produces the ordered
//! list of bus operations for each request so it can be unit-tested.

use crate::types::{EffectId, Rgb};

pub const I2C_ADDRESS: u8 = 0x68;

pub const PCI_VENDOR_NVIDIA: u16 = 0x10DE;
pub const PCI_SUBVENDOR_MSI: u16 = 0x1462;

/// Exact PCI identities this driver will talk to:
/// `(device, subsystem device, name)`. Nothing else is ever written to.
pub const SUPPORTED_CARDS: &[(u16, u16, &str)] = &[
    (0x2882, 0x5160, "MSI GeForce RTX 4060 GAMING X"),
    (0x2882, 0x5164, "MSI GeForce RTX 4060 GAMING X NV Edition"),
];

pub fn supported_card(
    vendor: u16,
    device: u16,
    subvendor: u16,
    subdevice: u16,
) -> Option<&'static str> {
    if vendor != PCI_VENDOR_NVIDIA || subvendor != PCI_SUBVENDOR_MSI {
        return None;
    }
    SUPPORTED_CARDS
        .iter()
        .find(|(d, s, _)| *d == device && *s == subdevice)
        .map(|(_, _, name)| *name)
}

pub mod reg {
    pub const MODE: u8 = 0x22;
    pub const COLOR_BLOCK1: u8 = 0x27;
    pub const COLOR_BLOCK2: u8 = 0x28;
    pub const UNKNOWN: u8 = 0x2E;
    pub const R1: u8 = 0x30;
    pub const G1: u8 = 0x31;
    pub const B1: u8 = 0x32;
    pub const BRIGHTNESS: u8 = 0x36;
    pub const SPEED: u8 = 0x38;
    pub const CONTROL: u8 = 0x46;
}

pub mod mode {
    pub const OFF: u8 = 0x01;
    pub const FLASHING: u8 = 0x02;
    pub const BREATHING: u8 = 0x04;
    pub const MAGIC: u8 = 0x07;
    pub const RAINBOW: u8 = 0x08;
    pub const STATIC: u8 = 0x13;
    pub const IDLE: u8 = 0x1C;
}

/// Full brightness (register value is 20 × level, level 1–5).
pub const BRIGHTNESS_MAX: u8 = 0x64;

/// 0–100 speed to the controller's three steps (0x04 slow … 0x01 fast).
pub fn speed_register(speed: u8) -> u8 {
    match speed {
        0..=33 => 0x04,
        34..=66 => 0x02,
        _ => 0x01,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// SMBus byte-data write `reg = value`.
    Write(u8, u8),
    /// I2C block write of 3 bytes (B, G, R) at `reg`.
    Block(u8, [u8; 3]),
}

pub fn unified_to_mode(effect: EffectId) -> Option<u8> {
    Some(match effect {
        EffectId::Off => mode::OFF,
        EffectId::Static => mode::STATIC,
        EffectId::Breathing => mode::BREATHING,
        EffectId::Flashing => mode::FLASHING,
        EffectId::SpectrumCycle => mode::MAGIC,
        EffectId::RainbowWave => mode::RAINBOW,
        EffectId::ColorPulse => return None,
    })
}

/// Ordered register writes to enter `mode` with colour `c` and `speed`.
pub fn sequence(mode_value: u8, c: Rgb, speed: u8) -> Vec<Op> {
    use Op::*;
    let s = speed_register(speed);
    let bgr = [c.b, c.g, c.r];
    let enter = [Write(reg::UNKNOWN, 0x00), Write(reg::MODE, mode::IDLE)];
    let mut ops = Vec::with_capacity(10);
    match mode_value {
        mode::STATIC => {
            ops.extend(enter);
            ops.extend([
                Write(reg::R1, c.r),
                Write(reg::G1, c.g),
                Write(reg::B1, c.b),
            ]);
            ops.push(Write(reg::BRIGHTNESS, BRIGHTNESS_MAX));
        }
        mode::FLASHING => {
            ops.extend(enter);
            ops.extend([
                Write(reg::R1, c.r),
                Write(reg::G1, c.g),
                Write(reg::B1, c.b),
            ]);
            ops.extend([Write(reg::BRIGHTNESS, BRIGHTNESS_MAX), Write(reg::SPEED, s)]);
        }
        mode::BREATHING => {
            ops.extend(enter);
            ops.extend([Block(reg::COLOR_BLOCK1, bgr), Block(reg::COLOR_BLOCK2, bgr)]);
            ops.extend([Write(reg::BRIGHTNESS, BRIGHTNESS_MAX), Write(reg::SPEED, s)]);
        }
        mode::MAGIC => {
            // Random-colour cycle; control register back to RGB mode.
            ops.extend(enter);
            ops.push(Write(reg::CONTROL, 0x00));
            ops.extend([Write(reg::BRIGHTNESS, BRIGHTNESS_MAX), Write(reg::SPEED, s)]);
        }
        mode::RAINBOW => {
            ops.push(Write(reg::CONTROL, 0x00)); // RGB mode, direction right
            ops.extend(enter);
            ops.extend([Write(reg::BRIGHTNESS, BRIGHTNESS_MAX), Write(reg::SPEED, s)]);
        }
        _ => ops.extend(enter), // OFF
    }
    ops.push(Write(reg::MODE, mode_value));
    ops
}

#[cfg(test)]
mod tests {
    use super::Op::*;
    use super::*;

    #[test]
    fn only_exact_cards_are_supported() {
        assert!(supported_card(0x10DE, 0x2882, 0x1462, 0x5160).is_some());
        assert!(supported_card(0x10DE, 0x2882, 0x1462, 0x5164).is_some());
        assert!(supported_card(0x10DE, 0x2882, 0x1462, 0x5161).is_none());
        assert!(supported_card(0x10DE, 0x2882, 0x1043, 0x5160).is_none());
        assert!(supported_card(0x1002, 0x2882, 0x1462, 0x5160).is_none());
    }

    #[test]
    fn static_sequence() {
        assert_eq!(
            sequence(mode::STATIC, Rgb::new(1, 2, 3), 50),
            vec![
                Write(0x2E, 0x00),
                Write(0x22, 0x1C),
                Write(0x30, 1),
                Write(0x31, 2),
                Write(0x32, 3),
                Write(0x36, 0x64),
                Write(0x22, 0x13),
            ]
        );
    }

    #[test]
    fn breathing_uses_bgr_blocks() {
        let ops = sequence(mode::BREATHING, Rgb::new(1, 2, 3), 100);
        assert_eq!(ops[2], Block(0x27, [3, 2, 1]));
        assert_eq!(ops[3], Block(0x28, [3, 2, 1]));
        assert_eq!(ops[5], Write(0x38, 0x01));
        assert_eq!(*ops.last().unwrap(), Write(0x22, 0x04));
    }

    #[test]
    fn rainbow_resets_control_first() {
        let ops = sequence(mode::RAINBOW, Rgb::WHITE, 0);
        assert_eq!(ops[0], Write(0x46, 0x00));
        assert!(ops.contains(&Write(0x38, 0x04)));
        assert_eq!(*ops.last().unwrap(), Write(0x22, 0x08));
    }

    #[test]
    fn every_write_targets_a_known_register() {
        let known = [0x22, 0x27, 0x28, 0x2E, 0x30, 0x31, 0x32, 0x36, 0x38, 0x46];
        for e in EffectId::ALL {
            let Some(m) = unified_to_mode(e) else {
                continue;
            };
            for op in sequence(m, Rgb::WHITE, 50) {
                let r = match op {
                    Write(r, _) | Block(r, _) => r,
                };
                assert!(known.contains(&r), "{e:?} wrote 0x{r:02x}");
            }
        }
    }
}
