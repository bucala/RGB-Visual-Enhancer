//! Logitech G peripherals (keyboards, mice, speakers) over USB HID using the
//! HID++ 2.0 lighting features. Works with wired devices (device index 0xFF)
//! and devices paired to a LIGHTSPEED/Unifying receiver (indices 1–6).

pub mod hidpp;
pub mod perkey;

#[cfg(feature = "hid")]
mod device;
#[cfg(feature = "hid")]
pub use device::{LogitechController, LogitechDetector};

pub const LOGITECH_VID: u16 = 0x046D;
/// Vendor-defined usage pages that carry HID++ reports: `0xFF00` on most
/// mice and receivers, `0xFF43` on G keyboards (G810/G910/G512…) and
/// headsets (G733/G933…).
pub const HIDPP_USAGE_PAGES: [u16; 2] = [0xFF00, 0xFF43];

/// Whether a HID collection carries 20-byte long (`0x11`) reports. Long
/// collections use usage `0x0002` on page `0xFF00` and `0x0602` / `0x0202`
/// on page `0xFF43`; the low byte is `0x02` in every case. (`0x01` is the
/// short-report collection, `0x04` the 64-byte one.)
pub fn is_long_report_collection(usage_page: u16, usage: u16) -> bool {
    HIDPP_USAGE_PAGES.contains(&usage_page) && usage & 0x00FF == 0x02
}

/// The 64-byte (`0x12`) collection used for per-key frames on G keyboards.
pub fn is_very_long_report_collection(usage_page: u16, usage: u16) -> bool {
    usage_page == 0xFF43 && usage & 0x00FF == 0x04
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_report_collections() {
        assert!(is_long_report_collection(0xFF00, 0x0002)); // G502
        assert!(is_long_report_collection(0xFF43, 0x0602)); // G910
        assert!(is_long_report_collection(0xFF43, 0x0202)); // G733
        assert!(!is_long_report_collection(0xFF00, 0x0001));
        assert!(!is_long_report_collection(0xFF43, 0x0604));
        assert!(!is_long_report_collection(0x0001, 0x0002));
    }
}
