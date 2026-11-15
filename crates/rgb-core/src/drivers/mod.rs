//! Driver families. Each exposes a [`DeviceDetector`](crate::DeviceDetector)
//! that yields [`RgbController`](crate::RgbController)s.

pub mod asus_aura;
#[cfg(feature = "hid")]
pub mod asus_aura_usb;
#[cfg(feature = "ble")]
pub mod ble;
#[cfg(feature = "hid")]
pub mod hid;
pub mod logitech;
#[cfg(feature = "gpu")]
pub mod msi_gpu;
pub mod virtual_device;
