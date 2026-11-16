//! MSI GeForce graphics card lighting.
//!
//! The RGB controller sits on the GPU's own I2C bus, which also carries the
//! voltage regulator, so this driver is deliberately strict: it only talks to
//! cards whose PCI identity is in [`protocol::SUPPORTED_CARDS`], only at
//! address 0x68 on port 1, only writes registers from the documented map,
//! reads a register before the first write, and never writes the save
//! (flash) register.
//!
//! Windows uses NvAPI from the NVIDIA driver. Linux is not wired up yet.

pub mod protocol;

#[cfg(windows)]
mod nvapi;

use std::collections::HashSet;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use self::protocol::Op;
use crate::controller::{DetectionReport, DeviceDetector, RgbController, RgbResult};
use crate::types::{DeviceInfo, EffectId, Rgb, ZoneInfo};

/// Register access on the GPU's RGB controller (address fixed by the backend).
pub trait GpuI2cBus: Send {
    fn read_byte(&mut self, reg: u8) -> io::Result<u8>;
    fn write_byte(&mut self, reg: u8, value: u8) -> io::Result<()>;
    fn write_block(&mut self, reg: u8, data: &[u8]) -> io::Result<()>;
}

/// The controller needs time between register writes.
const WRITE_GAP: Duration = Duration::from_millis(20);

type SharedBus = Arc<Mutex<Box<dyn GpuI2cBus>>>;

pub struct MsiGpuController {
    bus: SharedBus,
    info: DeviceInfo,
    color: Rgb,
    mode: u8,
    speed: u8,
    write_gap: Duration,
}

impl MsiGpuController {
    pub fn new(bus: Box<dyn GpuI2cBus>, info: DeviceInfo) -> Self {
        Self {
            bus: Arc::new(Mutex::new(bus)),
            info,
            color: Rgb::WHITE,
            mode: protocol::mode::STATIC,
            speed: 50,
            write_gap: WRITE_GAP,
        }
    }

    async fn run(&self, ops: Vec<Op>) -> RgbResult<()> {
        let bus = self.bus.clone();
        let gap = self.write_gap;
        tokio::task::spawn_blocking(move || {
            let mut bus = bus
                .lock()
                .map_err(|_| "GPU bus lock poisoned".to_string())?;
            for op in ops {
                match op {
                    Op::Write(r, v) => bus.write_byte(r, v),
                    Op::Block(r, d) => bus.write_block(r, &d),
                }
                .map_err(|e| format!("GPU I2C write failed: {e}"))?;
                std::thread::sleep(gap);
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("GPU worker panicked: {e}"))?
    }

    async fn apply(&self) -> RgbResult<()> {
        self.run(protocol::sequence(self.mode, self.color, self.speed))
            .await
    }
}

#[async_trait]
impl RgbController for MsiGpuController {
    async fn initialize(&mut self) -> RgbResult<()> {
        // Read-only presence check before we ever write.
        let bus = self.bus.clone();
        tokio::task::spawn_blocking(move || {
            let mut bus = bus
                .lock()
                .map_err(|_| "GPU bus lock poisoned".to_string())?;
            bus.read_byte(protocol::reg::MODE)
                .map_err(|e| format!("RGB controller not responding at 0x68: {e}"))
        })
        .await
        .map_err(|e| format!("GPU worker panicked: {e}"))??;

        self.info.zones = vec![ZoneInfo {
            index: 0,
            name: "GPU".into(),
            led_count: 0,
        }];
        self.info.effects = EffectId::ALL
            .into_iter()
            .filter(|e| protocol::unified_to_mode(*e).is_some())
            .map(EffectId::info)
            .collect();
        Ok(())
    }

    async fn set_color(&mut self, red: u8, green: u8, blue: u8) -> RgbResult<()> {
        self.color = Rgb::new(red, green, blue);
        if !matches!(
            self.mode,
            protocol::mode::BREATHING | protocol::mode::FLASHING
        ) {
            self.mode = protocol::mode::STATIC;
        }
        self.apply().await
    }

    async fn set_effect(&mut self, effect_id: u8, speed: u8) -> RgbResult<()> {
        let effect = EffectId::try_from(effect_id)?;
        self.mode = protocol::unified_to_mode(effect)
            .ok_or_else(|| format!("{} is not supported by MSI GPUs", effect.name()))?;
        self.speed = speed;
        self.apply().await
    }

    async fn get_device_info(&self) -> DeviceInfo {
        self.info.clone()
    }
}

/// Finds supported MSI cards among NVIDIA GPUs.
#[derive(Default)]
pub struct MsiGpuDetector;

#[cfg(windows)]
fn detect_blocking(known: &HashSet<String>) -> DetectionReport {
    let mut report = DetectionReport::default();
    let Some(gpus) = nvapi::enumerate() else {
        log::debug!("NvAPI unavailable; skipping NVIDIA GPU detection");
        return report;
    };
    for gpu in gpus {
        let ids = format!(
            "{:04x}:{:04x} subsystem {:04x}:{:04x}",
            gpu.vendor, gpu.device, gpu.subvendor, gpu.subdevice
        );
        let Some(name) =
            protocol::supported_card(gpu.vendor, gpu.device, gpu.subvendor, gpu.subdevice)
        else {
            if gpu.subvendor == protocol::PCI_SUBVENDOR_MSI {
                report.warn(format!("MSI GPU {ids} is not in the supported list yet"));
            }
            continue;
        };
        let id = format!("msi-gpu:{:04x}:{:04x}", gpu.device, gpu.subdevice);
        if known.contains(&id) {
            continue;
        }
        let bus = nvapi::NvI2c::new(&gpu, protocol::I2C_ADDRESS);
        report.controllers.push(Box::new(MsiGpuController::new(
            Box::new(bus),
            DeviceInfo {
                id,
                name: name.into(),
                vendor: "MSI".into(),
                device_type: crate::types::DeviceType::Gpu,
                transport: crate::types::Transport::I2c,
                location: format!("NvAPI I2C port 1 @ 0x68 ({ids})"),
                serial: None,
                firmware: None,
                zones: Vec::new(),
                effects: Vec::new(),
                layout: None,
            },
        )));
    }
    report
}

#[cfg(not(windows))]
fn detect_blocking(_known: &HashSet<String>) -> DetectionReport {
    DetectionReport::default()
}

#[async_trait]
impl DeviceDetector for MsiGpuDetector {
    fn name(&self) -> &'static str {
        "MSI GPU"
    }

    async fn detect(&self, known: &HashSet<String>) -> DetectionReport {
        let known = known.clone();
        tokio::task::spawn_blocking(move || detect_blocking(&known))
            .await
            .unwrap_or_else(|e| {
                let mut r = DetectionReport::default();
                r.warn(format!("GPU detection panicked: {e}"));
                r
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DeviceType, Transport};

    #[derive(Default, Clone)]
    struct Recorder(Arc<Mutex<Vec<String>>>);

    impl GpuI2cBus for Recorder {
        fn read_byte(&mut self, reg: u8) -> io::Result<u8> {
            self.0.lock().unwrap().push(format!("r{reg:02x}"));
            Ok(0x13)
        }
        fn write_byte(&mut self, reg: u8, value: u8) -> io::Result<()> {
            self.0
                .lock()
                .unwrap()
                .push(format!("{reg:02x}={value:02x}"));
            Ok(())
        }
        fn write_block(&mut self, reg: u8, data: &[u8]) -> io::Result<()> {
            self.0
                .lock()
                .unwrap()
                .push(format!("{reg:02x}<{data:02x?}"));
            Ok(())
        }
    }

    fn controller() -> (MsiGpuController, Recorder) {
        let rec = Recorder::default();
        let mut c = MsiGpuController::new(
            Box::new(rec.clone()),
            DeviceInfo {
                id: "msi-gpu:test".into(),
                name: "test".into(),
                vendor: "MSI".into(),
                device_type: DeviceType::Gpu,
                transport: Transport::I2c,
                location: "test".into(),
                serial: None,
                firmware: None,
                zones: vec![],
                effects: vec![],
                layout: None,
            },
        );
        c.write_gap = Duration::ZERO;
        (c, rec)
    }

    #[tokio::test]
    async fn reads_before_any_write_and_never_saves() {
        let (mut c, rec) = controller();
        c.initialize().await.unwrap();
        assert_eq!(rec.0.lock().unwrap().as_slice(), ["r22"]);
        c.set_color(255, 0, 0).await.unwrap();
        c.set_effect(EffectId::Breathing as u8, 80).await.unwrap();
        c.set_color(0, 0, 255).await.unwrap();
        let log = rec.0.lock().unwrap().clone();
        assert!(
            log.iter().all(|l| !l.starts_with("3f")),
            "save register written: {log:?}"
        );
        // Recolouring while breathing stays in breathing mode.
        assert_eq!(log.last().unwrap(), "22=04");
        assert!(log.contains(&"27<[ff, 00, 00]".to_string()), "{log:?}");
    }

    #[tokio::test]
    async fn rejects_unsupported_effect_without_writing() {
        let (mut c, rec) = controller();
        c.initialize().await.unwrap();
        assert!(c.set_effect(EffectId::ColorPulse as u8, 50).await.is_err());
        assert_eq!(rec.0.lock().unwrap().len(), 1);
    }
}
