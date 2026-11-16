//! Simulated devices. They behave like real controllers (latency, zones,
//! effect validation) so the UI and the manager can be exercised without
//! hardware. Enabled from the UI or with `RGBVE_VIRTUAL_DEVICES=1`.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::controller::{DetectionReport, DeviceDetector, RgbController, RgbResult};
use crate::types::{BatteryInfo, DeviceInfo, DeviceType, EffectId, Rgb, Transport, ZoneInfo};

pub const VIRTUAL_ID_PREFIX: &str = "virtual:";

pub struct VirtualController {
    info: DeviceInfo,
    latency: Duration,
    initialized: bool,
    pub color: Rgb,
    pub zone_colors: Vec<Rgb>,
    pub effect: Option<(EffectId, u8)>,
    pub battery: Option<BatteryInfo>,
    /// Per-key colours (virtual keyboards emulate a G910 layout).
    pub keys: std::collections::BTreeMap<String, Rgb>,
    /// Number of hardware writes performed (lets tests observe coalescing).
    pub writes: Arc<std::sync::atomic::AtomicUsize>,
}

impl VirtualController {
    pub fn new(
        slug: &str,
        name: &str,
        vendor: &str,
        device_type: DeviceType,
        zones: &[(&str, u16)],
        effects: &[EffectId],
    ) -> Self {
        Self {
            info: DeviceInfo {
                id: format!("{VIRTUAL_ID_PREFIX}{slug}"),
                name: name.to_string(),
                vendor: vendor.to_string(),
                device_type,
                transport: Transport::Virtual,
                location: "simulated".to_string(),
                serial: Some(format!("SIM-{slug}")),
                firmware: Some("sim-1.0".to_string()),
                zones: zones
                    .iter()
                    .enumerate()
                    .map(|(i, (n, leds))| ZoneInfo {
                        index: i as u8,
                        name: n.to_string(),
                        led_count: *leds,
                    })
                    .collect(),
                effects: effects.iter().map(|e| e.info()).collect(),
                layout: None,
            },
            latency: Duration::from_millis(8),
            initialized: false,
            color: Rgb::BLACK,
            zone_colors: vec![Rgb::BLACK; zones.len()],
            effect: None,
            battery: None,
            keys: Default::default(),
            writes: Arc::default(),
        }
    }

    pub fn with_layout(mut self, layout: &str) -> Self {
        self.info.layout = Some(layout.to_string());
        self
    }

    pub fn with_battery(mut self, percent: u8, charging: bool) -> Self {
        self.battery = Some(BatteryInfo {
            percent,
            charging,
            approximate: false,
        });
        self
    }

    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    async fn io(&self) -> RgbResult<()> {
        if !self.initialized {
            return Err("device not initialised".into());
        }
        tokio::time::sleep(self.latency).await;
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// A representative set of devices mirroring the supported hardware.
    pub fn demo_set() -> Vec<VirtualController> {
        use EffectId::*;
        vec![
            VirtualController::new(
                "aura-mobo",
                "Virtual ASUS Aura Motherboard",
                "ASUS",
                DeviceType::Motherboard,
                &[("Onboard", 6), ("ARGB Header 1", 60), ("ARGB Header 2", 30)],
                &[Off, Static, Breathing, Flashing, SpectrumCycle, RainbowWave],
            ),
            VirtualController::new(
                "g-keyboard",
                "Virtual Logitech G Keyboard",
                "Logitech",
                DeviceType::Keyboard,
                &[("Primary", 1), ("Logo", 1)],
                &[Off, Static, Breathing, SpectrumCycle, ColorPulse],
            )
            .with_layout(crate::drivers::logitech::perkey::LAYOUT_G910),
            VirtualController::new(
                "g-mouse",
                "Virtual Logitech G Mouse",
                "Logitech",
                DeviceType::Mouse,
                &[("Primary", 1), ("Logo", 1)],
                &[Off, Static, Breathing, SpectrumCycle],
            ),
            VirtualController::new(
                "g-headset",
                "Virtual Logitech G Headset",
                "Logitech",
                DeviceType::Headset,
                &[("Logo", 1), ("Strip", 1)],
                &[Off, Static, Breathing, SpectrumCycle],
            )
            .with_battery(64, false),
            VirtualController::new(
                "g-speakers",
                "Virtual Logitech G Speakers",
                "Logitech",
                DeviceType::Speaker,
                &[
                    ("Left", 1),
                    ("Right", 1),
                    ("Left Rear", 1),
                    ("Right Rear", 1),
                ],
                &[Off, Static, Breathing, SpectrumCycle],
            ),
            VirtualController::new(
                "ble-strip",
                "Virtual BLE LED Strip",
                "ESP32",
                DeviceType::LedStrip,
                &[("Strip", 120)],
                &[Off, Static, Breathing, SpectrumCycle, RainbowWave, Flashing],
            )
            .with_latency(Duration::from_millis(25))
            .with_battery(87, true),
        ]
    }
}

#[async_trait]
impl RgbController for VirtualController {
    async fn initialize(&mut self) -> RgbResult<()> {
        tokio::time::sleep(self.latency).await;
        self.initialized = true;
        Ok(())
    }

    async fn set_color(&mut self, red: u8, green: u8, blue: u8) -> RgbResult<()> {
        self.io().await?;
        self.color = Rgb::new(red, green, blue);
        self.zone_colors.fill(self.color);
        if !self.effect.is_some_and(|(e, _)| e.uses_color()) {
            self.effect = Some((EffectId::Static, 0));
        }
        Ok(())
    }

    async fn set_effect(&mut self, effect_id: u8, speed: u8) -> RgbResult<()> {
        let effect = EffectId::try_from(effect_id)?;
        if !self.info.effects.iter().any(|e| e.id == effect_id) {
            return Err(format!(
                "{} does not support {}",
                self.info.name,
                effect.name()
            ));
        }
        self.io().await?;
        self.effect = Some((effect, speed));
        Ok(())
    }

    async fn get_device_info(&self) -> DeviceInfo {
        self.info.clone()
    }

    async fn battery(&mut self) -> Option<BatteryInfo> {
        self.battery
    }

    async fn set_key_colors(&mut self, keys: &[(String, Rgb)]) -> RgbResult<()> {
        if self.info.layout.is_none() {
            return Err("per-key lighting is not supported by this device".into());
        }
        self.io().await?;
        self.keys.extend(keys.iter().cloned());
        Ok(())
    }

    async fn set_zone_color(&mut self, zone: u8, color: Rgb) -> RgbResult<()> {
        let slot = self
            .zone_colors
            .get_mut(zone as usize)
            .ok_or_else(|| format!("zone {zone} out of range"))?;
        *slot = color;
        self.io().await
    }
}

/// Produces [`VirtualController::demo_set`] while enabled.
pub struct VirtualDetector {
    enabled: Arc<AtomicBool>,
}

impl VirtualDetector {
    pub fn new(enabled: Arc<AtomicBool>) -> Self {
        Self { enabled }
    }
}

#[async_trait]
impl DeviceDetector for VirtualDetector {
    fn name(&self) -> &'static str {
        "Virtual devices"
    }

    async fn detect(&self, known: &HashSet<String>) -> DetectionReport {
        let mut report = DetectionReport::default();
        if !self.enabled.load(Ordering::SeqCst) {
            return report;
        }
        for device in VirtualController::demo_set() {
            if !known.contains(&device.info.id) {
                report.controllers.push(Box::new(device));
            }
        }
        report
    }
}
