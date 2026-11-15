use std::collections::{BTreeMap, HashSet};
use std::ffi::CString;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use hidapi::{HidApi, HidDevice};

use super::hidpp::{self, effect, LightingFeature, Request, Response, PARAMS_LONG};
use super::{is_long_report_collection, HIDPP_USAGE_PAGES, LOGITECH_VID};
use crate::controller::{DetectionReport, DeviceDetector, RgbController, RgbResult};
use crate::drivers::hid::SharedHidApi;
use crate::types::{scale_speed, DeviceInfo, DeviceType, EffectId, Rgb, Transport, ZoneInfo};

const REPLY_TIMEOUT: Duration = Duration::from_millis(600);
const PROBE_TIMEOUT: Duration = Duration::from_millis(200);
const RECEIVER_SLOTS: std::ops::RangeInclusive<u8> = 1..=6;

/// One opened HID interface. Several controllers share it when a receiver
/// hosts multiple paired devices; the mutex serialises request/response pairs.
pub struct HidppChannel {
    device: Mutex<HidDevice>,
    path: String,
}

impl HidppChannel {
    fn transact_blocking(&self, req: &Request, wait: Duration) -> RgbResult<[u8; PARAMS_LONG]> {
        let dev = self.device.lock().map_err(|_| "HID channel poisoned")?;
        dev.write(&req.encode())
            .map_err(|e| format!("HID write to {} failed: {e}", self.path))?;

        let deadline = Instant::now() + wait;
        let mut buf = [0u8; hidpp::LEN_VERY_LONG];
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("no HID++ reply (timeout)".into());
            }
            let n = dev
                .read_timeout(&mut buf, remaining.as_millis().max(1) as i32)
                .map_err(|e| format!("HID read from {} failed: {e}", self.path))?;
            if n == 0 {
                continue;
            }
            match req.match_response(&buf[..n]) {
                Response::Ok(params) => return Ok(params),
                Response::Error(code) => {
                    return Err(format!(
                        "HID++ error 0x{code:02x} ({})",
                        hidpp::error_name(code)
                    ))
                }
                // Notifications (battery, key events…) are interleaved; skip.
                Response::Unrelated => continue,
            }
        }
    }

    async fn request(self: &Arc<Self>, req: Request) -> RgbResult<[u8; PARAMS_LONG]> {
        let this = self.clone();
        tokio::task::spawn_blocking(move || this.transact_blocking(&req, REPLY_TIMEOUT))
            .await
            .map_err(|e| format!("HID worker panicked: {e}"))?
    }
}

struct LightingZone {
    index: u8,
    name: String,
    /// Effect type ids; the position in this list is the effect *index*
    /// the device expects in setZoneEffect.
    effects: Vec<u16>,
}

impl LightingZone {
    fn effect_index(&self, effect_type: u16) -> Option<u8> {
        self.effects
            .iter()
            .position(|e| *e == effect_type)
            .map(|i| i as u8)
    }
}

struct Lighting {
    feature: LightingFeature,
    feature_index: u8,
    zones: Vec<LightingZone>,
}

pub struct LogitechController {
    channel: Arc<HidppChannel>,
    device_index: u8,
    info: DeviceInfo,
    lighting: Option<Lighting>,
    color: Rgb,
    /// Active effect type and period, re-applied when the colour changes.
    active: (u16, u16),
}

fn unified_to_logitech(effect: EffectId) -> Option<u16> {
    Some(match effect {
        EffectId::Off => effect::DISABLED,
        EffectId::Static => effect::STATIC,
        EffectId::Breathing => effect::BREATHE,
        EffectId::SpectrumCycle => effect::CYCLE,
        EffectId::ColorPulse => effect::PULSE,
        EffectId::RainbowWave | EffectId::Flashing => return None,
    })
}

impl LogitechController {
    fn new(channel: Arc<HidppChannel>, device_index: u8, info: DeviceInfo) -> Self {
        Self {
            channel,
            device_index,
            info,
            lighting: None,
            color: Rgb::WHITE,
            active: (effect::STATIC, 0),
        }
    }

    async fn feature_index(&self, feature: u16) -> RgbResult<Option<u8>> {
        let r = self
            .channel
            .request(hidpp::get_feature(self.device_index, feature))
            .await?;
        Ok((r[0] != 0).then_some(r[0]))
    }

    async fn read_name(&self, feat: u8) -> RgbResult<(String, u8)> {
        let dev = self.device_index;
        let len = self
            .channel
            .request(Request::new(dev, feat, 0, &[]))
            .await?[0] as usize;
        let mut name = Vec::with_capacity(len);
        while name.len() < len {
            let chunk = self
                .channel
                .request(Request::new(dev, feat, 1, &[name.len() as u8]))
                .await?;
            let take = (len - name.len()).min(PARAMS_LONG);
            if chunk[..take].iter().all(|b| *b == 0) {
                break;
            }
            name.extend_from_slice(&chunk[..take]);
        }
        let kind = self
            .channel
            .request(Request::new(dev, feat, 2, &[]))
            .await
            .map(|r| r[0])
            .unwrap_or(0xFF);
        let name = String::from_utf8_lossy(&name)
            .trim_end_matches('\0')
            .trim()
            .to_string();
        Ok((name, kind))
    }

    /// All feature ids the device implements (via `IFeatureSet`, 0x0001).
    /// Used for diagnostics when a device lacks a supported lighting feature.
    async fn list_features(&self) -> RgbResult<Vec<u16>> {
        let feat = self
            .feature_index(hidpp::FEATURE_FEATURE_SET)
            .await?
            .ok_or("no IFeatureSet")?;
        let dev = self.device_index;
        let count = self
            .channel
            .request(Request::new(dev, feat, 0, &[]))
            .await?[0];
        let mut ids = Vec::with_capacity(count as usize);
        for i in 1..=count {
            let r = self
                .channel
                .request(Request::new(dev, feat, 1, &[i]))
                .await?;
            ids.push(u16::from_be_bytes([r[0], r[1]]));
        }
        Ok(ids)
    }

    async fn discover_lighting(&self) -> RgbResult<Lighting> {
        let (feature, feature_index) = match self
            .feature_index(hidpp::FEATURE_COLOR_LED_EFFECTS)
            .await?
        {
            Some(i) => (LightingFeature::ColorLedEffects, i),
            None => match self.feature_index(hidpp::FEATURE_RGB_EFFECTS).await? {
                Some(i) => (LightingFeature::RgbEffects, i),
                None => {
                    let features = match self.list_features().await {
                        Ok(f) => f
                            .iter()
                            .map(|f| format!("0x{f:04x}"))
                            .collect::<Vec<_>>()
                            .join(", "),
                        Err(e) => format!("unavailable ({e})"),
                    };
                    return Err(format!(
                        "no supported HID++ lighting feature (0x8070/0x8071); device features: {features}"
                    ));
                }
            },
        };
        let dev = self.device_index;
        let info = self
            .channel
            .request(feature.info_request(dev, feature_index))
            .await?;
        let zone_count = feature.parse_zone_count(&info);

        let mut zones = Vec::with_capacity(zone_count as usize);
        for z in 0..zone_count {
            let zi = self
                .channel
                .request(feature.zone_info_request(dev, feature_index, z))
                .await?;
            let (location, effect_count) = feature.parse_zone_info(&zi);
            let mut effects = Vec::with_capacity(effect_count as usize);
            for e in 0..effect_count {
                let ei = self
                    .channel
                    .request(feature.effect_info_request(dev, feature_index, z, e))
                    .await?;
                effects.push(feature.parse_effect_info(&ei));
            }
            zones.push(LightingZone {
                index: z,
                name: hidpp::zone_location_name(location, z),
                effects,
            });
        }
        Ok(Lighting {
            feature,
            feature_index,
            zones,
        })
    }

    async fn apply(
        &self,
        zone: &LightingZone,
        effect_type: u16,
        color: Rgb,
        period_ms: u16,
    ) -> RgbResult<()> {
        let lighting = self.lighting.as_ref().ok_or("lighting not initialised")?;
        let index = zone
            .effect_index(effect_type)
            .ok_or_else(|| format!("zone '{}' lacks effect 0x{effect_type:02x}", zone.name))?;
        let params = hidpp::effect_params(effect_type, (color.r, color.g, color.b), period_ms);
        let req = lighting.feature.set_effect_request(
            self.device_index,
            lighting.feature_index,
            zone.index,
            index,
            &params,
        );
        self.channel.request(req).await.map(|_| ())
    }

    /// Apply an effect to every zone that supports it.
    async fn apply_all(&self, effect_type: u16, color: Rgb, period_ms: u16) -> RgbResult<()> {
        let lighting = self.lighting.as_ref().ok_or("lighting not initialised")?;
        let mut applied = 0;
        let mut last_err = None;
        for zone in lighting
            .zones
            .iter()
            .filter(|z| z.effect_index(effect_type).is_some())
        {
            match self.apply(zone, effect_type, color, period_ms).await {
                Ok(()) => applied += 1,
                Err(e) => last_err = Some(e),
            }
        }
        match (applied, last_err) {
            (0, Some(e)) => Err(e),
            (0, None) => Err("no zone supports this effect".into()),
            _ => Ok(()),
        }
    }
}

#[async_trait]
impl RgbController for LogitechController {
    async fn initialize(&mut self) -> RgbResult<()> {
        if let Some(feat) = self.feature_index(hidpp::FEATURE_DEVICE_NAME).await? {
            if let Ok((name, kind)) = self.read_name(feat).await {
                if !name.is_empty() {
                    self.info.name = if name.contains("Logitech") {
                        name
                    } else {
                        format!("Logitech {name}")
                    };
                }
                let kind = hidpp::device_kind(kind);
                if kind != DeviceType::Unknown {
                    self.info.device_type = kind;
                }
            }
        }

        let lighting = self.discover_lighting().await?;

        // Stop onboard profiles from overriding us. Not every device has
        // the feature, and failure here is not fatal.
        if let Ok(Some(feat)) = self.feature_index(hidpp::FEATURE_ONBOARD_PROFILES).await {
            if let Err(e) = self
                .channel
                .request(hidpp::set_host_mode(self.device_index, feat))
                .await
            {
                log::debug!("{}: could not enter host mode: {e}", self.info.name);
            }
        }

        self.info.zones = lighting
            .zones
            .iter()
            .map(|z| ZoneInfo {
                index: z.index,
                name: z.name.clone(),
                led_count: 1,
            })
            .collect();
        self.info.effects = EffectId::ALL
            .into_iter()
            .filter(|e| {
                unified_to_logitech(*e)
                    .is_some_and(|t| lighting.zones.iter().any(|z| z.effects.contains(&t)))
            })
            .map(EffectId::info)
            .collect();
        self.lighting = Some(lighting);
        Ok(())
    }

    async fn set_color(&mut self, red: u8, green: u8, blue: u8) -> RgbResult<()> {
        self.color = Rgb::new(red, green, blue);
        let (effect_type, period) = match self.active {
            (t @ (effect::BREATHE | effect::PULSE), period) => (t, period),
            _ => (effect::STATIC, 0),
        };
        self.apply_all(effect_type, self.color, period).await?;
        self.active = (effect_type, period);
        Ok(())
    }

    async fn set_effect(&mut self, effect_id: u8, speed: u8) -> RgbResult<()> {
        let effect = EffectId::try_from(effect_id)?;
        let effect_type = unified_to_logitech(effect)
            .ok_or_else(|| format!("{} is not supported by HID++ lighting", effect.name()))?;
        let period = scale_speed(speed, 20_000, 1_000) as u16;
        self.apply_all(effect_type, self.color, period).await?;
        self.active = (effect_type, period);
        Ok(())
    }

    async fn get_device_info(&self) -> DeviceInfo {
        self.info.clone()
    }

    async fn set_zone_color(&mut self, zone: u8, color: Rgb) -> RgbResult<()> {
        let lighting = self.lighting.as_ref().ok_or("lighting not initialised")?;
        let z = lighting
            .zones
            .iter()
            .find(|z| z.index == zone)
            .ok_or_else(|| format!("zone {zone} out of range"))?;
        self.apply(z, effect::STATIC, color, 0).await
    }
}

/// Enumerates HID++ interfaces of Logitech devices.
#[derive(Default)]
pub struct LogitechDetector {
    api: SharedHidApi,
}

impl LogitechDetector {
    pub fn new(api: SharedHidApi) -> Self {
        Self { api }
    }
}

struct Candidate {
    path: CString,
    path_display: String,
    pid: u16,
    key: String,
    product: String,
    serial: Option<String>,
}

fn guess_type(product: &str) -> DeviceType {
    let p = product.to_ascii_lowercase();
    if p.contains("keyboard") {
        DeviceType::Keyboard
    } else if p.contains("mouse") {
        DeviceType::Mouse
    } else if p.contains("speaker") {
        DeviceType::Speaker
    } else if p.contains("headset") {
        DeviceType::Headset
    } else {
        DeviceType::Unknown
    }
}

fn detect_blocking(
    api: &mut HidApi,
    warnings: &mut Vec<String>,
    known: &HashSet<String>,
) -> Vec<Box<dyn RgbController>> {
    let mut controllers: Vec<Box<dyn RgbController>> = Vec::new();

    // One candidate per physical HID++ interface. On Windows each top-level
    // collection is its own path, so pick the long-report one; on Linux all
    // usages share a hidraw node and dedupe by path.
    let mut candidates: BTreeMap<String, Candidate> = BTreeMap::new();
    for d in api.device_list() {
        if d.vendor_id() != LOGITECH_VID || !HIDPP_USAGE_PAGES.contains(&d.usage_page()) {
            continue;
        }
        if cfg!(windows) && !is_long_report_collection(d.usage_page(), d.usage()) {
            continue;
        }
        let path_display = d.path().to_string_lossy().into_owned();
        let serial = d
            .serial_number()
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        let key = serial.clone().unwrap_or_else(|| path_display.clone());
        candidates.entry(path_display.clone()).or_insert(Candidate {
            path: d.path().to_owned(),
            path_display,
            pid: d.product_id(),
            key,
            product: d.product_string().unwrap_or("Logitech device").to_string(),
            serial,
        });
    }

    for c in candidates.into_values() {
        let prefix = format!("logitech:{LOGITECH_VID:04x}:{:04x}:{}:", c.pid, c.key);
        if known.iter().any(|k| k.starts_with(&prefix)) {
            continue;
        }
        let device = match api.open_path(&c.path) {
            Ok(d) => d,
            Err(e) => {
                let hint = if cfg!(target_os = "linux") {
                    " (install packaging/linux/60-rgb-visual-enhancer.rules for hidraw access)"
                } else {
                    ""
                };
                warnings.push(format!(
                    "cannot open {} ({}): {e}{hint}",
                    c.product, c.path_display
                ));
                continue;
            }
        };
        let channel = Arc::new(HidppChannel {
            device: Mutex::new(device),
            path: c.path_display.clone(),
        });

        // Wired devices answer on 0xFF. Receivers answer 0xFF with a HID++
        // 1.0 error, so probe their pairing slots instead.
        let indices: Vec<u8> = match channel
            .transact_blocking(&hidpp::ping(hidpp::DEVICE_INDEX_WIRED, 0x5A), PROBE_TIMEOUT)
        {
            Ok(_) => vec![hidpp::DEVICE_INDEX_WIRED],
            Err(_) => RECEIVER_SLOTS
                .filter(|i| {
                    channel
                        .transact_blocking(&hidpp::ping(*i, 0x5A), PROBE_TIMEOUT)
                        .is_ok()
                })
                .collect(),
        };

        for index in indices {
            let info = DeviceInfo {
                id: format!("{prefix}{index:02x}"),
                name: c.product.clone(),
                vendor: "Logitech".into(),
                device_type: guess_type(&c.product),
                transport: Transport::Usb,
                location: if index == hidpp::DEVICE_INDEX_WIRED {
                    c.path_display.clone()
                } else {
                    format!("{} (receiver slot {index})", c.path_display)
                },
                serial: c.serial.clone(),
                firmware: None,
                zones: Vec::new(),
                effects: Vec::new(),
            };
            controllers.push(Box::new(LogitechController::new(
                channel.clone(),
                index,
                info,
            )));
        }
    }
    controllers
}

#[async_trait]
impl DeviceDetector for LogitechDetector {
    fn name(&self) -> &'static str {
        "Logitech HID++"
    }

    async fn detect(&self, known: &HashSet<String>) -> DetectionReport {
        let api = self.api.clone();
        let known = known.clone();
        let result = tokio::task::spawn_blocking(move || {
            api.with_refreshed(|api, warnings| {
                let controllers = detect_blocking(api, warnings, &known);
                (controllers, std::mem::take(warnings))
            })
        })
        .await;
        match result {
            Ok(Ok((controllers, warnings))) => DetectionReport {
                controllers,
                warnings,
            },
            Ok(Err(e)) => {
                let mut r = DetectionReport::default();
                r.warn(e);
                r
            }
            Err(e) => {
                let mut r = DetectionReport::default();
                r.warn(format!("HID detection panicked: {e}"));
                r
            }
        }
    }
}
