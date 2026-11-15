//! ENE SMBus RGB controller protocol, used by ASUS Aura motherboards (and
//! Aura DRAM). Register map and access pattern as documented by OpenRGB's
//! `ENESMBusController`.
//!
//! Registers are 16-bit and accessed indirectly:
//!
//! 1. write word to command `0x00` with the register address **byte-swapped**
//! 2. read byte from command `0x81`, write byte to `0x01`, or block-write to `0x03`
//!
//! Colours are stored in **R, B, G** order.

use std::io;

use super::smbus::SmbusBus;
use crate::types::{EffectId, Rgb};

pub const REG_DEVICE_NAME: u16 = 0x1000;
pub const REG_CONFIG_TABLE: u16 = 0x1C00;
pub const REG_COLORS_DIRECT: u16 = 0x8000;
pub const REG_COLORS_EFFECT: u16 = 0x8010;
pub const REG_DIRECT: u16 = 0x8020;
pub const REG_MODE: u16 = 0x8021;
pub const REG_SPEED: u16 = 0x8022;
pub const REG_DIRECTION: u16 = 0x8023;
pub const REG_APPLY: u16 = 0x80A0;
pub const REG_COLORS_DIRECT_V2: u16 = 0x8100;
pub const REG_COLORS_EFFECT_V2: u16 = 0x8160;

pub const APPLY_VAL: u8 = 0x01;

pub const CONFIG_LED_COUNT: usize = 0x02;
pub const CONFIG_CHANNEL_V1: usize = 0x13;
pub const CONFIG_CHANNEL_V2: usize = 0x1B;

pub const CMD_SET_ADDR: u8 = 0x00;
pub const CMD_WRITE_BYTE: u8 = 0x01;
pub const CMD_WRITE_BLOCK: u8 = 0x03;
pub const CMD_READ_BYTE: u8 = 0x81;

/// 7-bit addresses where ASUS places the onboard ENE controller.
pub const MOTHERBOARD_ADDRESSES: [u8; 3] = [0x40, 0x4E, 0x4F];

pub mod mode {
    pub const OFF: u8 = 0;
    pub const STATIC: u8 = 1;
    pub const BREATHING: u8 = 2;
    pub const FLASHING: u8 = 3;
    pub const SPECTRUM_CYCLE: u8 = 4;
    pub const RAINBOW: u8 = 5;
}

/// Hardware speed register: 0x00 = fastest … 0x04 = slowest.
pub fn speed_register(speed: u8) -> u8 {
    (4 - crate::types::scale_speed(speed, 0, 4)) as u8
}

pub fn unified_to_mode(effect: EffectId) -> Option<u8> {
    Some(match effect {
        EffectId::Off => mode::OFF,
        EffectId::Static => mode::STATIC,
        EffectId::Breathing => mode::BREATHING,
        EffectId::Flashing => mode::FLASHING,
        EffectId::SpectrumCycle => mode::SPECTRUM_CYCLE,
        EffectId::RainbowWave => mode::RAINBOW,
        EffectId::ColorPulse => return None,
    })
}

/// Register layout generation, chosen from the device name string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    V1,
    V2,
}

impl Layout {
    pub fn from_name(name: &str) -> Self {
        if name.starts_with("AUMA0-E6K5") {
            Layout::V2
        } else {
            Layout::V1
        }
    }

    pub fn effect_colors(self) -> u16 {
        match self {
            Layout::V1 => REG_COLORS_EFFECT,
            Layout::V2 => REG_COLORS_EFFECT_V2,
        }
    }

    pub fn direct_colors(self) -> u16 {
        match self {
            Layout::V1 => REG_COLORS_DIRECT,
            Layout::V2 => REG_COLORS_DIRECT_V2,
        }
    }

    pub fn channel_offset(self) -> usize {
        match self {
            Layout::V1 => CONFIG_CHANNEL_V1,
            Layout::V2 => CONFIG_CHANNEL_V2,
        }
    }

    /// Hardware limit on LEDs addressable through the colour registers.
    pub fn max_leds(self) -> usize {
        match self {
            Layout::V1 => 5,
            Layout::V2 => 10,
        }
    }
}

/// Register-level access to one ENE controller.
pub struct Ene<'a> {
    pub bus: &'a mut dyn SmbusBus,
    pub addr: u8,
}

impl Ene<'_> {
    fn set_register(&mut self, reg: u16) -> io::Result<()> {
        self.bus
            .write_word_data(self.addr, CMD_SET_ADDR, reg.swap_bytes())
    }

    pub fn read(&mut self, reg: u16) -> io::Result<u8> {
        self.set_register(reg)?;
        self.bus.read_byte_data(self.addr, CMD_READ_BYTE)
    }

    pub fn write(&mut self, reg: u16, value: u8) -> io::Result<()> {
        self.set_register(reg)?;
        self.bus.write_byte_data(self.addr, CMD_WRITE_BYTE, value)
    }

    pub fn write_block(&mut self, reg: u16, data: &[u8]) -> io::Result<()> {
        // Keep chunks a multiple of 3 so a colour never straddles two writes.
        for (i, chunk) in data.chunks(30).enumerate() {
            self.set_register(reg + (i * 30) as u16)?;
            self.bus
                .write_block_data(self.addr, CMD_WRITE_BLOCK, chunk)?;
        }
        Ok(())
    }

    pub fn read_string(&mut self, reg: u16, len: usize) -> io::Result<String> {
        let mut bytes = Vec::with_capacity(len);
        for i in 0..len {
            let b = self.read(reg + i as u16)?;
            if b == 0 {
                break;
            }
            bytes.push(b);
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    pub fn read_config_table(&mut self) -> io::Result<[u8; 64]> {
        let mut table = [0u8; 64];
        for (i, slot) in table.iter_mut().enumerate() {
            *slot = self.read(REG_CONFIG_TABLE + i as u16)?;
        }
        Ok(table)
    }

    /// ENE controllers return 0x00..0x0F when reading commands 0xA0..0xAF.
    /// This fingerprint is what keeps us from poking unrelated chips.
    pub fn fingerprint(bus: &mut dyn SmbusBus, addr: u8) -> bool {
        if bus.read_byte(addr).is_err() {
            return false;
        }
        (0xA0u8..0xB0).all(|cmd| bus.read_byte_data(addr, cmd).ok() == Some(cmd - 0xA0))
    }
}

/// Encode colours in the controller's R-B-G byte order.
pub fn encode_colors(colors: &[Rgb]) -> Vec<u8> {
    colors.iter().flat_map(|c| [c.r, c.b, c.g]).collect()
}

/// Human-readable names for the channel ids in the config table.
pub fn channel_name(id: u8) -> &'static str {
    match id {
        0x05 | 0x0D => "Audio",
        0x06 => "Backplate",
        0x07 => "Back I/O",
        0x08 => "Center",
        0x09 => "Center Start",
        0x0B => "PCIe",
        0x0C => "Chipset",
        0x0E => "Addressable Header",
        0x82 => "RGB Header",
        0x83 => "RGB Header 2",
        0x84 => "RGB Header 3",
        _ => "LED",
    }
}

#[cfg(test)]
pub mod fake {
    //! An in-memory ENE controller for tests.
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    pub struct FakeEne {
        pub addr: u8,
        pub regs: HashMap<u16, u8>,
        pointer: u16,
        pub log: Vec<String>,
    }

    impl FakeEne {
        pub fn new(addr: u8, name: &str, leds: u8) -> Self {
            let mut regs = HashMap::new();
            for (i, b) in name.bytes().enumerate() {
                regs.insert(REG_DEVICE_NAME + i as u16, b);
            }
            regs.insert(REG_CONFIG_TABLE + CONFIG_LED_COUNT as u16, leds);
            let layout = Layout::from_name(name);
            for i in 0..leds {
                regs.insert(
                    REG_CONFIG_TABLE + (layout.channel_offset() as u16) + i as u16,
                    0x82,
                );
            }
            Self {
                addr,
                regs,
                ..Default::default()
            }
        }
    }

    impl SmbusBus for FakeEne {
        fn name(&self) -> &str {
            "fake"
        }
        fn read_byte(&mut self, addr: u8) -> io::Result<u8> {
            if addr == self.addr {
                Ok(0)
            } else {
                Err(io::ErrorKind::NotFound.into())
            }
        }
        fn read_byte_data(&mut self, addr: u8, cmd: u8) -> io::Result<u8> {
            if addr != self.addr {
                return Err(io::ErrorKind::NotFound.into());
            }
            Ok(match cmd {
                0xA0..=0xAF => cmd - 0xA0,
                CMD_READ_BYTE => *self.regs.get(&self.pointer).unwrap_or(&0),
                _ => 0xFF,
            })
        }
        fn write_byte_data(&mut self, _addr: u8, cmd: u8, value: u8) -> io::Result<()> {
            assert_eq!(cmd, CMD_WRITE_BYTE);
            self.regs.insert(self.pointer, value);
            self.log.push(format!("{:04x}={value:02x}", self.pointer));
            Ok(())
        }
        fn write_word_data(&mut self, _addr: u8, cmd: u8, value: u16) -> io::Result<()> {
            assert_eq!(cmd, CMD_SET_ADDR);
            self.pointer = value.swap_bytes();
            Ok(())
        }
        fn write_block_data(&mut self, _addr: u8, cmd: u8, data: &[u8]) -> io::Result<()> {
            assert_eq!(cmd, CMD_WRITE_BLOCK);
            for (i, b) in data.iter().enumerate() {
                self.regs.insert(self.pointer + i as u16, *b);
            }
            self.log.push(format!("{:04x}<-{data:02x?}", self.pointer));
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeEne;
    use super::*;

    #[test]
    fn register_address_is_byte_swapped() {
        assert_eq!(REG_MODE.swap_bytes(), 0x2180);
    }

    #[test]
    fn rbg_encoding() {
        assert_eq!(encode_colors(&[Rgb::new(1, 2, 3)]), vec![1, 3, 2]);
    }

    #[test]
    fn speed_mapping() {
        assert_eq!(speed_register(0), 4);
        assert_eq!(speed_register(100), 0);
        assert_eq!(speed_register(50), 2);
    }

    #[test]
    fn fingerprint_and_reads() {
        let mut bus = FakeEne::new(0x4E, "AUMA0-E6K5-0106", 6);
        assert!(Ene::fingerprint(&mut bus, 0x4E));
        assert!(!Ene::fingerprint(&mut bus, 0x40));
        let mut ene = Ene {
            bus: &mut bus,
            addr: 0x4E,
        };
        assert_eq!(
            ene.read_string(REG_DEVICE_NAME, 16).unwrap(),
            "AUMA0-E6K5-0106"
        );
        assert_eq!(ene.read_config_table().unwrap()[CONFIG_LED_COUNT], 6);
    }

    #[test]
    fn block_writes_are_chunked_on_color_boundaries() {
        let mut bus = FakeEne::new(0x4E, "AUMA0-E6K5-0106", 10);
        let colors = vec![Rgb::new(9, 8, 7); 12];
        Ene {
            bus: &mut bus,
            addr: 0x4E,
        }
        .write_block(REG_COLORS_EFFECT_V2, &encode_colors(&colors))
        .unwrap();
        assert_eq!(bus.log.len(), 2);
        assert!(bus.log[0].starts_with("8160<-"));
        assert!(bus.log[1].starts_with("817e<-"));
        // Last byte of the 12th colour is its green component (R, B, G order).
        assert_eq!(bus.regs[&(REG_COLORS_EFFECT_V2 + 35)], 8);
    }
}
