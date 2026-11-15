//! # rgb-core
//!
//! Vendor-neutral RGB device control. The crate is UI-agnostic: the Tauri app
//! (or a CLI, or tests) builds a [`DeviceManager`] from a set of
//! [`DeviceDetector`]s and drives devices through it.
//!
//! ```text
//!  UI / IPC ──► DeviceManager ──► per-device worker task ──► RgbController
//!                    │                                        ├─ Logitech HID++ (USB HID)
//!                    └── broadcast<ManagerEvent> ──► UI        ├─ ASUS Aura (SMBus / ENE)
//!                                                             ├─ BLE GATT strips
//!                                                             └─ Virtual (simulated)
//! ```

pub mod controller;
pub mod drivers;
pub mod manager;
pub mod system;
pub mod types;

pub use controller::{DetectionReport, DeviceDetector, RgbController, RgbResult};
pub use manager::{
    DeviceManager, DeviceSnapshot, DeviceStatus, ManagerConfig, ManagerEvent, ScanSummary,
};
pub use types::{
    BatteryInfo, DeviceInfo, DeviceType, EffectId, EffectInfo, Rgb, Transport, ZoneInfo,
};

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// All hardware detectors compiled into this build, plus the virtual-device
/// detector gated by `virtual_enabled`.
pub fn default_detectors(virtual_enabled: Arc<AtomicBool>) -> Vec<Arc<dyn DeviceDetector>> {
    let mut detectors: Vec<Arc<dyn DeviceDetector>> = vec![Arc::new(
        drivers::virtual_device::VirtualDetector::new(virtual_enabled),
    )];
    #[cfg(feature = "hid")]
    {
        let hid = drivers::hid::SharedHidApi::default();
        detectors.push(Arc::new(drivers::logitech::LogitechDetector::new(
            hid.clone(),
        )));
        detectors.push(Arc::new(drivers::asus_aura_usb::AuraUsbDetector::new(hid)));
    }
    #[cfg(feature = "smbus")]
    detectors.push(Arc::new(drivers::asus_aura::AuraSmbusDetector));
    #[cfg(feature = "gpu")]
    detectors.push(Arc::new(drivers::msi_gpu::MsiGpuDetector));
    #[cfg(feature = "ble")]
    detectors.push(Arc::new(drivers::ble::BleDetector::default()));
    detectors
}
