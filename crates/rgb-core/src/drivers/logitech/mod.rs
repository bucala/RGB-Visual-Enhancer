//! Logitech G peripherals (keyboards, mice, speakers) over USB HID using the
//! HID++ 2.0 lighting features. Works with wired devices (device index 0xFF)
//! and devices paired to a LIGHTSPEED/Unifying receiver (indices 1–6).

pub mod hidpp;

#[cfg(feature = "hid")]
mod device;
#[cfg(feature = "hid")]
pub use device::{LogitechController, LogitechDetector};

pub const LOGITECH_VID: u16 = 0x046D;
/// Vendor-defined usage page that carries HID++ reports.
pub const HIDPP_USAGE_PAGE: u16 = 0xFF00;
/// Usage of the long-report (0x11) collection.
pub const HIDPP_USAGE_LONG: u16 = 0x0002;
