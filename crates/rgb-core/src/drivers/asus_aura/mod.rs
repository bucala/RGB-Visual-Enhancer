//! ASUS Aura motherboard lighting through the ENE controller on the chipset
//! SMBus.
//!
//! Safety first: SMBus is shared with RAM SPD, sensors and VRM controllers.
//! We only probe the documented Aura addresses, only on ASUS boards, and only
//! talk to a device after it passes the ENE fingerprint check. DRAM modules
//! (which require address remapping) are intentionally not touched.

pub mod protocol;
pub mod smbus;

use std::collections::HashSet;
use std::io;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use self::protocol::{Ene, Layout};
use self::smbus::SmbusBus;
use crate::controller::{DetectionReport, DeviceDetector, RgbController, RgbResult};
use crate::types::{DeviceInfo, DeviceType, EffectId, Rgb, Transport, ZoneInfo};

pub const ASUS_PCI_SUBSYSTEM_VENDOR: u16 = 0x1043;

type SharedBus = Arc<Mutex<Box<dyn SmbusBus>>>;

pub struct AuraSmbusController {
    bus: SharedBus,
    addr: u8,
    layout: Layout,
    colors: Vec<Rgb>,
    mode: u8,
    info: DeviceInfo,
}

impl AuraSmbusController {
    pub fn new(bus: SharedBus, bus_name: &str, addr: u8, id: String) -> Self {
        Self {
            bus,
            addr,
            layout: Layout::V1,
            colors: Vec::new(),
            mode: protocol::mode::STATIC,
            info: DeviceInfo {
                id,
                name: "ASUS Aura Motherboard".into(),
                vendor: "ASUS".into(),
                device_type: DeviceType::Motherboard,
                transport: Transport::Smbus,
                location: format!("{bus_name} @ 0x{addr:02x}"),
                serial: None,
                firmware: None,
                zones: Vec::new(),
                effects: Vec::new(),
            },
        }
    }

    /// Run register I/O on the blocking pool.
    async fn with_ene<T, F>(&self, f: F) -> RgbResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Ene<'_>) -> io::Result<T> + Send + 'static,
    {
        let bus = self.bus.clone();
        let addr = self.addr;
        tokio::task::spawn_blocking(move || {
            let mut guard = bus.lock().map_err(|_| "SMBus lock poisoned".to_string())?;
            let mut ene = Ene {
                bus: guard.as_mut(),
                addr,
            };
            f(&mut ene).map_err(|e| format!("SMBus I/O error: {e}"))
        })
        .await
        .map_err(|e| format!("SMBus worker panicked: {e}"))?
    }

    async fn write_colors_and_mode(&self, mode: u8, speed: Option<u8>) -> RgbResult<()> {
        let data = protocol::encode_colors(&self.colors);
        let reg = self.layout.effect_colors();
        self.with_ene(move |ene| {
            ene.write(protocol::REG_DIRECT, 0x00)?;
            ene.write_block(reg, &data)?;
            ene.write(protocol::REG_MODE, mode)?;
            if let Some(s) = speed {
                ene.write(protocol::REG_SPEED, s)?;
            }
            ene.write(protocol::REG_APPLY, protocol::APPLY_VAL)
        })
        .await
    }
}

#[async_trait]
impl RgbController for AuraSmbusController {
    async fn initialize(&mut self) -> RgbResult<()> {
        let (name, table) = self
            .with_ene(|ene| {
                let name = ene.read_string(protocol::REG_DEVICE_NAME, 16)?;
                let table = ene.read_config_table()?;
                Ok((name, table))
            })
            .await?;

        self.layout = Layout::from_name(&name);
        let led_count = (table[protocol::CONFIG_LED_COUNT] as usize).min(self.layout.max_leds());
        if led_count == 0 {
            return Err(format!("ENE controller '{name}' reports no LEDs"));
        }
        let offset = self.layout.channel_offset();
        self.info.zones = (0..led_count)
            .map(|i| ZoneInfo {
                index: i as u8,
                name: format!("{} {}", protocol::channel_name(table[offset + i]), i + 1),
                led_count: 1,
            })
            .collect();
        self.info.firmware = Some(name);
        self.info.effects = EffectId::ALL
            .into_iter()
            .filter(|e| protocol::unified_to_mode(*e).is_some())
            .map(EffectId::info)
            .collect();
        self.colors = vec![Rgb::WHITE; led_count];
        Ok(())
    }

    async fn set_color(&mut self, red: u8, green: u8, blue: u8) -> RgbResult<()> {
        self.colors.fill(Rgb::new(red, green, blue));
        // A solid colour implies Static unless a colour-aware effect is running.
        if !matches!(
            self.mode,
            protocol::mode::BREATHING | protocol::mode::FLASHING
        ) {
            self.mode = protocol::mode::STATIC;
        }
        self.write_colors_and_mode(self.mode, None).await
    }

    async fn set_effect(&mut self, effect_id: u8, speed: u8) -> RgbResult<()> {
        let effect = EffectId::try_from(effect_id)?;
        let mode = protocol::unified_to_mode(effect)
            .ok_or_else(|| format!("{} is not supported by Aura SMBus", effect.name()))?;
        self.mode = mode;
        self.write_colors_and_mode(mode, Some(protocol::speed_register(speed)))
            .await
    }

    async fn get_device_info(&self) -> DeviceInfo {
        self.info.clone()
    }

    async fn set_zone_color(&mut self, zone: u8, color: Rgb) -> RgbResult<()> {
        let slot = self
            .colors
            .get_mut(zone as usize)
            .ok_or_else(|| format!("zone {zone} out of range"))?;
        *slot = color;
        self.write_colors_and_mode(self.mode, None).await
    }
}

/// Probes chipset SMBus adapters for ASUS Aura ENE controllers.
#[derive(Default)]
pub struct AuraSmbusDetector;

fn detect_blocking(known: &HashSet<String>) -> (Vec<Box<dyn RgbController>>, Vec<String>) {
    let mut controllers: Vec<Box<dyn RgbController>> = Vec::new();
    let mut warnings = Vec::new();

    let board_vendor = crate::system::SystemInfo::collect().board_vendor;
    let asus_board = board_vendor
        .as_deref()
        .is_some_and(|v| v.to_ascii_uppercase().contains("ASUS"));

    let buses = match smbus::enumerate_buses() {
        Ok(b) => b,
        Err(e) => {
            // Only worth a warning on ASUS boards; elsewhere it's expected.
            if asus_board {
                warnings.push(e);
            }
            return (controllers, warnings);
        }
    };

    for desc in buses {
        let is_asus = match desc.subsystem_vendor {
            Some(v) => v == ASUS_PCI_SUBSYSTEM_VENDOR,
            None => asus_board,
        };
        if !is_asus {
            log::debug!("skipping {}: not an ASUS SMBus controller", desc.name);
            continue;
        }
        let short = desc
            .name
            .split_whitespace()
            .next()
            .unwrap_or("i2c")
            .to_string();
        let pending: Vec<u8> = protocol::MOTHERBOARD_ADDRESSES
            .into_iter()
            .filter(|a| !known.contains(&format!("aura-smbus:{short}:{a:02x}")))
            .collect();
        if pending.is_empty() {
            continue;
        }
        let name = desc.name.clone();
        let mut bus = match (desc.open)() {
            Ok(b) => b,
            Err(e) => {
                let hint = if e.kind() == io::ErrorKind::PermissionDenied {
                    " (grant access via packaging/linux/60-rgb-visual-enhancer.rules or run as root)"
                } else {
                    ""
                };
                warnings.push(format!("cannot open {name}: {e}{hint}"));
                continue;
            }
        };
        let found: Vec<u8> = pending
            .into_iter()
            .filter(|a| Ene::fingerprint(bus.as_mut(), *a))
            .collect();
        if found.is_empty() {
            continue;
        }
        let shared: SharedBus = Arc::new(Mutex::new(bus));
        for addr in found {
            let id = format!("aura-smbus:{short}:{addr:02x}");
            controllers.push(Box::new(AuraSmbusController::new(
                shared.clone(),
                &name,
                addr,
                id,
            )));
        }
    }
    (controllers, warnings)
}

#[async_trait]
impl DeviceDetector for AuraSmbusDetector {
    fn name(&self) -> &'static str {
        "ASUS Aura SMBus"
    }

    async fn detect(&self, known: &HashSet<String>) -> DetectionReport {
        let known = known.clone();
        match tokio::task::spawn_blocking(move || detect_blocking(&known)).await {
            Ok((controllers, warnings)) => DetectionReport {
                controllers,
                warnings,
            },
            Err(e) => {
                let mut r = DetectionReport::default();
                r.warn(format!("SMBus detection panicked: {e}"));
                r
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::protocol::fake::FakeEne;
    use super::*;

    fn controller(name: &str, leds: u8) -> (AuraSmbusController, SharedBus) {
        let bus: SharedBus = Arc::new(Mutex::new(Box::new(FakeEne::new(0x4E, name, leds))));
        (
            AuraSmbusController::new(bus.clone(), "i2c-0", 0x4E, "aura-smbus:i2c-0:4e".into()),
            bus,
        )
    }

    #[tokio::test]
    async fn initialises_from_config_table() {
        let (mut c, _) = controller("AUMA0-E6K5-0106", 7);
        c.initialize().await.unwrap();
        let info = c.get_device_info().await;
        assert_eq!(info.zones.len(), 7);
        assert_eq!(info.firmware.as_deref(), Some("AUMA0-E6K5-0106"));
        assert!(info
            .effects
            .iter()
            .any(|e| e.id == EffectId::RainbowWave as u8));
        assert!(!info
            .effects
            .iter()
            .any(|e| e.id == EffectId::ColorPulse as u8));
    }

    #[tokio::test]
    async fn set_color_writes_rbg_then_static_and_apply() {
        let (mut c, bus) = controller("LED-0116", 3);
        c.initialize().await.unwrap();
        c.set_color(10, 20, 30).await.unwrap();

        // Read the fake controller's registers back through the protocol layer.
        let mut guard = bus.lock().unwrap();
        let mut ene = Ene {
            bus: guard.as_mut(),
            addr: 0x4E,
        };
        let reg = protocol::REG_COLORS_EFFECT;
        let got: Vec<u8> = (0..9).map(|i| ene.read(reg + i).unwrap()).collect();
        assert_eq!(got, vec![10, 30, 20, 10, 30, 20, 10, 30, 20]);
        assert_eq!(
            ene.read(protocol::REG_MODE).unwrap(),
            protocol::mode::STATIC
        );
        assert_eq!(ene.read(protocol::REG_APPLY).unwrap(), protocol::APPLY_VAL);
        assert_eq!(ene.read(protocol::REG_DIRECT).unwrap(), 0);
    }

    #[tokio::test]
    async fn effect_sets_mode_and_speed() {
        let (mut c, bus) = controller("AUMA0-E6K5-0106", 2);
        c.initialize().await.unwrap();
        c.set_effect(EffectId::SpectrumCycle as u8, 100)
            .await
            .unwrap();
        {
            let mut guard = bus.lock().unwrap();
            let mut ene = Ene {
                bus: guard.as_mut(),
                addr: 0x4E,
            };
            assert_eq!(
                ene.read(protocol::REG_MODE).unwrap(),
                protocol::mode::SPECTRUM_CYCLE
            );
            assert_eq!(ene.read(protocol::REG_SPEED).unwrap(), 0);
        }
        assert!(c.set_effect(EffectId::ColorPulse as u8, 50).await.is_err());
    }
}
