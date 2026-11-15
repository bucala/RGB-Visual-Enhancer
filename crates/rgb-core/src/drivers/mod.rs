//! Driver families. Each exposes a [`DeviceDetector`](crate::DeviceDetector)
//! that yields [`RgbController`](crate::RgbController)s.

pub mod asus_aura;
#[cfg(feature = "ble")]
pub mod ble;
pub mod logitech;
pub mod virtual_device;
