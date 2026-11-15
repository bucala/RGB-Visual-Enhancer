//! Bluetooth Low Energy LED controllers via `btleplug`.

pub mod profiles;

use std::collections::HashSet;
use std::time::Duration;

use async_trait::async_trait;
use btleplug::api::{
    Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use tokio::sync::Mutex;

use self::profiles::BleProfile;
use crate::controller::{DetectionReport, DeviceDetector, RgbController, RgbResult};
use crate::types::{DeviceInfo, DeviceType, EffectId, Rgb, Transport, ZoneInfo};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct BleController {
    peripheral: Peripheral,
    profile: BleProfile,
    characteristic: Option<Characteristic>,
    info: DeviceInfo,
    color: Rgb,
    powered: Option<bool>,
    /// Active animated effect, re-sent with the new colour when possible.
    active: Option<(EffectId, u8)>,
}

impl BleController {
    async fn connect(&mut self) -> RgbResult<()> {
        let p = &self.peripheral;
        if !p.is_connected().await.unwrap_or(false) {
            tokio::time::timeout(CONNECT_TIMEOUT, p.connect())
                .await
                .map_err(|_| "BLE connect timed out".to_string())?
                .map_err(|e| format!("BLE connect failed: {e}"))?;
            self.powered = None;
        }
        if self.characteristic.is_none() {
            p.discover_services()
                .await
                .map_err(|e| format!("GATT discovery failed: {e}"))?;
            let wanted = self.profile.characteristic();
            self.characteristic = Some(
                p.characteristics()
                    .into_iter()
                    .find(|c| c.uuid == wanted)
                    .ok_or_else(|| format!("control characteristic {wanted} not found"))?,
            );
        }
        Ok(())
    }

    async fn write(&mut self, frames: &[Vec<u8>]) -> RgbResult<()> {
        // Transparent reconnect: BLE links drop on their own regularly.
        self.connect().await?;
        let ch = self.characteristic.as_ref().expect("connected above");
        let kind = if ch
            .properties
            .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
        {
            WriteType::WithoutResponse
        } else {
            WriteType::WithResponse
        };
        for frame in frames {
            if let Err(e) = self.peripheral.write(ch, frame, kind).await {
                // Force rediscovery next time; the handle may be stale.
                self.characteristic = None;
                return Err(format!("BLE write failed: {e}"));
            }
        }
        Ok(())
    }

    async fn ensure_power(&mut self, on: bool) -> RgbResult<()> {
        if self.powered != Some(on) {
            self.write(&[self.profile.power(on)]).await?;
            self.powered = Some(on);
        }
        Ok(())
    }
}

#[async_trait]
impl RgbController for BleController {
    async fn initialize(&mut self) -> RgbResult<()> {
        self.connect().await
    }

    async fn set_color(&mut self, red: u8, green: u8, blue: u8) -> RgbResult<()> {
        self.color = Rgb::new(red, green, blue);
        self.ensure_power(true).await?;
        if let Some((effect, speed)) = self.active.filter(|(e, _)| e.uses_color()) {
            if let Some(frames) = self.profile.effect(effect, speed, self.color) {
                return self.write(&frames).await;
            }
        }
        self.active = None;
        let frame = self.profile.color(self.color);
        self.write(&[frame]).await
    }

    async fn set_effect(&mut self, effect_id: u8, speed: u8) -> RgbResult<()> {
        let effect = EffectId::try_from(effect_id)?;
        match effect {
            EffectId::Off => self.ensure_power(false).await,
            EffectId::Static => {
                self.active = None;
                let c = self.color;
                self.set_color(c.r, c.g, c.b).await
            }
            _ => {
                let frames = self
                    .profile
                    .effect(effect, speed, self.color)
                    .ok_or_else(|| {
                        format!(
                            "{} is not supported by {}",
                            effect.name(),
                            self.profile.label()
                        )
                    })?;
                self.ensure_power(true).await?;
                self.write(&frames).await?;
                self.active = Some((effect, speed));
                Ok(())
            }
        }
    }

    async fn get_device_info(&self) -> DeviceInfo {
        self.info.clone()
    }

    async fn shutdown(&mut self) -> RgbResult<()> {
        if self.peripheral.is_connected().await.unwrap_or(false) {
            self.peripheral
                .disconnect()
                .await
                .map_err(|e| format!("BLE disconnect failed: {e}"))?;
        }
        Ok(())
    }
}

/// Scans for known BLE LED controllers.
pub struct BleDetector {
    adapter: Mutex<Option<Adapter>>,
    scan_duration: Duration,
}

impl Default for BleDetector {
    fn default() -> Self {
        Self {
            adapter: Mutex::new(None),
            scan_duration: Duration::from_secs(5),
        }
    }
}

impl BleDetector {
    async fn adapter(&self) -> Result<Adapter, String> {
        let mut slot = self.adapter.lock().await;
        if let Some(a) = slot.as_ref() {
            return Ok(a.clone());
        }
        let manager = Manager::new()
            .await
            .map_err(|e| format!("Bluetooth unavailable: {e}"))?;
        let adapter = manager
            .adapters()
            .await
            .map_err(|e| format!("cannot list Bluetooth adapters: {e}"))?
            .into_iter()
            .next()
            .ok_or("no Bluetooth adapter found")?;
        *slot = Some(adapter.clone());
        Ok(adapter)
    }
}

#[async_trait]
impl DeviceDetector for BleDetector {
    fn name(&self) -> &'static str {
        "Bluetooth LE"
    }

    async fn detect(&self, known: &HashSet<String>) -> DetectionReport {
        let mut report = DetectionReport::default();
        let adapter = match self.adapter().await {
            Ok(a) => a,
            Err(e) => {
                report.warn(e);
                return report;
            }
        };
        if let Err(e) = adapter.start_scan(ScanFilter::default()).await {
            report.warn(format!("BLE scan failed: {e}"));
            return report;
        }
        tokio::time::sleep(self.scan_duration).await;
        let _ = adapter.stop_scan().await;

        let peripherals = match adapter.peripherals().await {
            Ok(p) => p,
            Err(e) => {
                report.warn(format!("cannot list BLE peripherals: {e}"));
                return report;
            }
        };
        for peripheral in peripherals {
            let Ok(Some(props)) = peripheral.properties().await else {
                continue;
            };
            let name = props
                .local_name
                .clone()
                .or(props.advertisement_name.clone());
            let Some(profile) = BleProfile::identify(name.as_deref(), &props.services) else {
                continue;
            };

            // Some platforms (macOS) hide the MAC address; fall back to the
            // opaque peripheral id.
            let address = if props.address.into_inner() == [0; 6] {
                format!("{:?}", peripheral.id())
            } else {
                props.address.to_string()
            };
            let id = format!("ble:{address}");
            if known.contains(&id) {
                continue;
            }
            let display = name.unwrap_or_else(|| format!("{} light", profile.label()));
            let info = DeviceInfo {
                id,
                name: display.trim().to_string(),
                vendor: profile.label().into(),
                device_type: DeviceType::LedStrip,
                transport: Transport::Ble,
                location: address,
                serial: None,
                firmware: None,
                zones: vec![ZoneInfo {
                    index: 0,
                    name: "All LEDs".into(),
                    led_count: 0,
                }],
                effects: profile
                    .supported_effects()
                    .iter()
                    .map(|e| e.info())
                    .collect(),
            };
            report.controllers.push(Box::new(BleController {
                peripheral,
                profile,
                characteristic: None,
                info,
                color: Rgb::WHITE,
                powered: None,
                active: None,
            }));
        }
        report
    }
}
