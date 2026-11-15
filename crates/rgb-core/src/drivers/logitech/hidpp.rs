//! Logitech HID++ 2.0 framing and the lighting-related features.
//!
//! This module is pure (no I/O) so packet layouts can be unit-tested.
//!
//! Message layout (long report, 20 bytes):
//!
//! ```text
//! [0x11] [device index] [feature index] [function << 4 | sw id] [16 bytes params]
//! ```
//!
//! Feature indices are per-device and discovered through the root feature
//! (`IRoot`, always at index 0). Lighting uses:
//!
//! * `0x8070` COLOR_LED_EFFECTS – most G keyboards/mice/speakers
//!   (fn 0 getInfo, fn 1 getZoneInfo, fn 2 getZoneEffectInfo, fn 3 setZoneEffect)
//! * `0x8071` RGB_EFFECTS – newer devices (fn 0 info, fn 1 setRgbClusterEffect)
//! * `0x8100` ONBOARD_PROFILES – switch to host mode so firmware profiles don't
//!   override software-set lighting.
//!
//! References: Solaar (`logitech_receiver/hidpp20.py`) and OpenRGB's Logitech
//! controllers, which document the same layouts.

pub const REPORT_SHORT: u8 = 0x10;
pub const REPORT_LONG: u8 = 0x11;
pub const REPORT_VERY_LONG: u8 = 0x12;
pub const LEN_SHORT: usize = 7;
pub const LEN_LONG: usize = 20;
pub const LEN_VERY_LONG: usize = 64;
pub const PARAMS_LONG: usize = LEN_LONG - 4;

/// Device index for a device attached directly over USB (not via a receiver).
pub const DEVICE_INDEX_WIRED: u8 = 0xFF;
/// Software id stamped into requests so we can recognise our own replies.
pub const SW_ID: u8 = 0x0A;
/// Feature index the device uses to report HID++ 2.0 errors.
pub const ERROR_FEATURE_INDEX: u8 = 0xFF;
/// HID++ 1.0 error sub-id (returned e.g. by receivers for unpaired slots).
pub const HIDPP10_ERROR: u8 = 0x8F;

pub const FEATURE_ROOT: u16 = 0x0000;
pub const FEATURE_DEVICE_NAME: u16 = 0x0005;
pub const FEATURE_COLOR_LED_EFFECTS: u16 = 0x8070;
pub const FEATURE_RGB_EFFECTS: u16 = 0x8071;
pub const FEATURE_ONBOARD_PROFILES: u16 = 0x8100;

/// Logitech effect type ids (from getZoneEffectInfo).
pub mod effect {
    pub const DISABLED: u16 = 0x00;
    pub const STATIC: u16 = 0x01;
    pub const PULSE: u16 = 0x02;
    pub const CYCLE: u16 = 0x03;
    pub const BREATHE: u16 = 0x0A;
    pub const RIPPLE: u16 = 0x0B;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub device_index: u8,
    pub feature_index: u8,
    pub function: u8,
    pub params: [u8; PARAMS_LONG],
}

impl Request {
    pub fn new(device_index: u8, feature_index: u8, function: u8, params: &[u8]) -> Self {
        assert!(function < 16, "HID++ function ids are 4 bits");
        assert!(
            params.len() <= PARAMS_LONG,
            "too many params for a long report"
        );
        let mut p = [0u8; PARAMS_LONG];
        p[..params.len()].copy_from_slice(params);
        Self {
            device_index,
            feature_index,
            function,
            params: p,
        }
    }

    pub fn function_byte(&self) -> u8 {
        (self.function << 4) | SW_ID
    }

    /// Encode as a long (0x11) report.
    pub fn encode(&self) -> [u8; LEN_LONG] {
        let mut buf = [0u8; LEN_LONG];
        buf[0] = REPORT_LONG;
        buf[1] = self.device_index;
        buf[2] = self.feature_index;
        buf[3] = self.function_byte();
        buf[4..].copy_from_slice(&self.params);
        buf
    }

    /// Classify an incoming report relative to this request.
    pub fn match_response(&self, report: &[u8]) -> Response {
        if report.len() < 4 || ![REPORT_SHORT, REPORT_LONG, REPORT_VERY_LONG].contains(&report[0]) {
            return Response::Unrelated;
        }
        if report[1] != self.device_index {
            return Response::Unrelated;
        }
        // HID++ 2.0 error: [rid, dev, 0xFF, feature idx, fn|sw, code]
        if report[2] == ERROR_FEATURE_INDEX
            && report.len() >= 6
            && report[3] == self.feature_index
            && report[4] == self.function_byte()
        {
            return Response::Error(report[5]);
        }
        // HID++ 1.0 error: [rid, dev, 0x8F, sub id, addr, code]
        if report[2] == HIDPP10_ERROR && report.len() >= 6 && report[3] == self.feature_index {
            return Response::Error(report[5]);
        }
        if report[2] == self.feature_index && report[3] == self.function_byte() {
            let mut params = [0u8; PARAMS_LONG];
            let body = &report[4..report.len().min(4 + PARAMS_LONG)];
            params[..body.len()].copy_from_slice(body);
            return Response::Ok(params);
        }
        Response::Unrelated
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Ok([u8; PARAMS_LONG]),
    Error(u8),
    /// A notification or a reply to someone else's request.
    Unrelated,
}

pub fn error_name(code: u8) -> &'static str {
    match code {
        0x01 => "unknown",
        0x02 => "invalid argument",
        0x03 => "out of range",
        0x04 => "hardware error",
        0x05 => "logitech internal",
        0x06 => "invalid feature index",
        0x07 => "invalid function id",
        0x08 => "busy",
        0x09 => "unsupported",
        _ => "unrecognised error",
    }
}

/// `IRoot.getFeature(feature_id)` – reply param 0 is the feature index (0 = absent).
pub fn get_feature(device_index: u8, feature: u16) -> Request {
    let [hi, lo] = feature.to_be_bytes();
    Request::new(device_index, 0x00, 0, &[hi, lo])
}

/// `IRoot.getProtocolVersion` with a ping byte – used to probe a device index.
pub fn ping(device_index: u8, token: u8) -> Request {
    Request::new(device_index, 0x00, 1, &[0, 0, token])
}

/// Which lighting feature a device exposes; the two share effect parameter
/// layouts but differ in function numbers and info reply offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightingFeature {
    ColorLedEffects,
    RgbEffects,
}

impl LightingFeature {
    pub fn id(self) -> u16 {
        match self {
            Self::ColorLedEffects => FEATURE_COLOR_LED_EFFECTS,
            Self::RgbEffects => FEATURE_RGB_EFFECTS,
        }
    }

    pub fn info_request(self, dev: u8, feat: u8) -> Request {
        match self {
            Self::ColorLedEffects => Request::new(dev, feat, 0, &[]),
            Self::RgbEffects => Request::new(dev, feat, 0, &[0xFF, 0xFF, 0x00]),
        }
    }

    pub fn parse_zone_count(self, r: &[u8; PARAMS_LONG]) -> u8 {
        match self {
            Self::ColorLedEffects => r[0],
            Self::RgbEffects => r[2],
        }
    }

    pub fn zone_info_request(self, dev: u8, feat: u8, zone: u8) -> Request {
        match self {
            Self::ColorLedEffects => Request::new(dev, feat, 1, &[zone, 0xFF, 0x00]),
            Self::RgbEffects => Request::new(dev, feat, 0, &[zone, 0xFF, 0x00]),
        }
    }

    /// Returns `(location, effect_count)`.
    pub fn parse_zone_info(self, r: &[u8; PARAMS_LONG]) -> (u16, u8) {
        let o = match self {
            Self::ColorLedEffects => 1,
            Self::RgbEffects => 2,
        };
        (u16::from_be_bytes([r[o], r[o + 1]]), r[o + 2])
    }

    pub fn effect_info_request(self, dev: u8, feat: u8, zone: u8, effect_index: u8) -> Request {
        let function = match self {
            Self::ColorLedEffects => 2,
            Self::RgbEffects => 0,
        };
        Request::new(dev, feat, function, &[zone, effect_index, 0x00])
    }

    /// Returns the effect *type* id at the queried index.
    pub fn parse_effect_info(self, r: &[u8; PARAMS_LONG]) -> u16 {
        u16::from_be_bytes([r[2], r[3]])
    }

    /// Build a set-effect request. `effect_index` is the position in the
    /// zone's effect list (not the effect type id).
    pub fn set_effect_request(
        self,
        dev: u8,
        feat: u8,
        zone: u8,
        effect_index: u8,
        params: &[u8; 10],
    ) -> Request {
        let mut p = [0u8; 13];
        p[0] = zone;
        p[1] = effect_index;
        p[2..12].copy_from_slice(params);
        match self {
            // Byte 12 left 0: apply to RAM only, don't wear the flash.
            Self::ColorLedEffects => Request::new(dev, feat, 3, &p),
            Self::RgbEffects => {
                p[12] = 0x01;
                Request::new(dev, feat, 1, &p)
            }
        }
    }
}

/// `OnboardProfiles.setOnboardMode(host)` so firmware stops applying its own
/// lighting profile.
pub fn set_host_mode(dev: u8, feat: u8) -> Request {
    Request::new(dev, feat, 1, &[0x02])
}

/// 10-byte effect parameter block for an effect type.
/// `period_ms` applies to animated effects; `intensity` is 1–100 (0 = default).
pub fn effect_params(effect_type: u16, color: (u8, u8, u8), period_ms: u16) -> [u8; 10] {
    let (r, g, b) = color;
    let [ph, pl] = period_ms.to_be_bytes();
    let mut p = [0u8; 10];
    match effect_type {
        effect::STATIC => {
            p[..3].copy_from_slice(&[r, g, b]);
        }
        effect::PULSE => {
            // Pulse takes a speed byte, larger = faster. Derive it from the period.
            let speed = (255 - (u32::from(period_ms).min(20_000) * 254 / 20_000)) as u8;
            p[..4].copy_from_slice(&[r, g, b, speed]);
        }
        effect::CYCLE => {
            p[5] = ph;
            p[6] = pl;
            p[7] = 100; // intensity
        }
        effect::BREATHE => {
            p[..7].copy_from_slice(&[r, g, b, ph, pl, 0x00, 100]);
        }
        effect::RIPPLE => {
            p[..3].copy_from_slice(&[r, g, b]);
            p[4] = ph;
            p[5] = pl;
        }
        _ => {}
    }
    p
}

pub fn zone_location_name(location: u16, index: u8) -> String {
    match location {
        0x0001 => "Primary".into(),
        0x0002 => "Logo".into(),
        0x0003 => "Left Side".into(),
        0x0004 => "Right Side".into(),
        0x0005 => "Combined".into(),
        0x0006..=0x000A => format!("Primary {}", location - 0x0005),
        _ => format!("Zone {}", index + 1),
    }
}

/// `x0005 DeviceName.getDeviceType` kinds we care about.
pub fn device_kind(kind: u8) -> crate::types::DeviceType {
    use crate::types::DeviceType::*;
    match kind {
        0x00 | 0x02 => Keyboard,
        0x03..=0x05 => Mouse,
        0x08 => Headset,
        0x0E => Speaker,
        0x10 => Light,
        _ => Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_get_feature() {
        let r = get_feature(0xFF, FEATURE_COLOR_LED_EFFECTS).encode();
        assert_eq!(&r[..6], &[0x11, 0xFF, 0x00, 0x0A, 0x80, 0x70]);
        assert!(r[6..].iter().all(|b| *b == 0));
    }

    #[test]
    fn matches_success_error_and_unrelated() {
        let req = Request::new(0xFF, 0x0E, 3, &[]);
        let mut ok = [0u8; 20];
        ok[..5].copy_from_slice(&[0x11, 0xFF, 0x0E, 0x3A, 0x42]);
        match req.match_response(&ok) {
            Response::Ok(p) => assert_eq!(p[0], 0x42),
            other => panic!("unexpected {other:?}"),
        }

        let err = [0x11, 0xFF, 0xFF, 0x0E, 0x3A, 0x02, 0x00];
        assert_eq!(req.match_response(&err), Response::Error(0x02));

        // Same feature, different function -> not ours.
        let other = [0x11, 0xFF, 0x0E, 0x1A, 0x00, 0x00, 0x00];
        assert_eq!(req.match_response(&other), Response::Unrelated);
        // Different device index.
        let other_dev = [0x11, 0x01, 0x0E, 0x3A, 0x00, 0x00, 0x00];
        assert_eq!(req.match_response(&other_dev), Response::Unrelated);
        // Short report replies are accepted too.
        let short = [0x10, 0xFF, 0x0E, 0x3A, 0x07, 0x00, 0x00];
        assert!(matches!(req.match_response(&short), Response::Ok(p) if p[0] == 7));
    }

    #[test]
    fn color_led_set_static_layout() {
        let params = effect_params(effect::STATIC, (0x12, 0x34, 0x56), 0);
        let r = LightingFeature::ColorLedEffects
            .set_effect_request(0xFF, 0x0C, 1, 2, &params)
            .encode();
        assert_eq!(
            &r[..10],
            &[0x11, 0xFF, 0x0C, 0x3A, 0x01, 0x02, 0x12, 0x34, 0x56, 0x00]
        );
        assert_eq!(r[16], 0x00, "RAM only, no persistence flag");
    }

    #[test]
    fn rgb_effects_set_layout_and_persistence() {
        let params = effect_params(effect::BREATHE, (1, 2, 3), 0x1388);
        let r = LightingFeature::RgbEffects
            .set_effect_request(0x01, 0x10, 0, 4, &params)
            .encode();
        assert_eq!(
            &r[..13],
            &[0x11, 0x01, 0x10, 0x1A, 0x00, 0x04, 1, 2, 3, 0x13, 0x88, 0x00, 100]
        );
        assert_eq!(r[16], 0x01);
    }

    #[test]
    fn cycle_params() {
        let p = effect_params(effect::CYCLE, (9, 9, 9), 5000);
        assert_eq!(p, [0, 0, 0, 0, 0, 0x13, 0x88, 100, 0, 0]);
    }

    #[test]
    fn info_parsing_offsets() {
        let mut r = [0u8; PARAMS_LONG];
        r[..4].copy_from_slice(&[0x00, 0x00, 0x02, 0x05]);
        assert_eq!(
            LightingFeature::ColorLedEffects.parse_zone_info(&r),
            (0x0002, 0x05)
        );
        r[..5].copy_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x07]);
        assert_eq!(
            LightingFeature::RgbEffects.parse_zone_info(&r),
            (0x0001, 0x07)
        );
    }
}
