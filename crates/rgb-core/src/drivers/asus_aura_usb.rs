//! ASUS Aura **USB** mainboard controller (`0B05:18F3/1939/19AF/1AA6/1BED`).
//!
//! AMD 500-series and Intel 400-series-and-newer ASUS boards (e.g. ROG STRIX
//! B550-F) drive their onboard LEDs and RGB/ARGB headers from this USB HID
//! controller instead of the SMBus ENE chip. Unlike SMBus it needs no kernel
//! driver on Windows.
//!
//! Protocol (as documented by OpenRGB's `AuraMainboardController`): 65-byte
//! reports with report id `0xEC`.
//!
//! | Cmd | Layout | Meaning |
//! |---|---|---|
//! | `0x82` | – | firmware string request (reply `EC 02 <ascii>`) |
//! | `0xB0` | – | config table request (reply `EC 30 .. .. <60 bytes>`) |
//! | `0x35` | `ch 00 00 mode` | select effect mode for an effect channel |
//! | `0x36` | `mask_hi mask_lo 00 <rgb…>` | colours for effect mode, one slot per LED |
//! | `0x40` | `flags|ch start count <rgb…>` | direct colours; flag `0x80` = apply |
//!
//! Effect channel 0 is the mainboard (onboard LEDs followed by 12 V RGB
//! headers); ARGB header *i* is effect channel *i + 1* and direct channel
//! *i*. The mainboard's direct channel is `0x04`.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use hidapi::{HidApi, HidDevice};

use crate::controller::{DetectionReport, DeviceDetector, RgbController, RgbResult};
use crate::drivers::hid::SharedHidApi;
use crate::types::{DeviceInfo, DeviceType, EffectId, Rgb, Transport, ZoneInfo};

pub const ASUS_VID: u16 = 0x0B05;
pub const MAINBOARD_PIDS: [u16; 5] = [0x18F3, 0x1939, 0x19AF, 0x1AA6, 0x1BED];
pub const AURA_USAGE_PAGE: u16 = 0xFF72;

pub const REPORT_ID: u8 = 0xEC;
pub const REPORT_LEN: usize = 65;

pub const CMD_FIRMWARE: u8 = 0x82;
pub const CMD_CONFIG_TABLE: u8 = 0xB0;
pub const CMD_EFFECT: u8 = 0x35;
pub const CMD_EFFECT_COLOR: u8 = 0x36;
pub const CMD_DIRECT: u8 = 0x40;
pub const REPLY_FIRMWARE: u8 = 0x02;
pub const REPLY_CONFIG_TABLE: u8 = 0x30;

pub const MAINBOARD_DIRECT_CHANNEL: u8 = 0x04;
pub const DIRECT_APPLY: u8 = 0x80;
pub const DIRECT_MAX_LEDS: usize = (REPORT_LEN - 5) / 3; // 20
/// Effect-colour slots are addressed by a 16-bit mask.
pub const EFFECT_MAX_SLOTS: usize = 16;
/// ARGB strips don't report their length; direct writes cover this many.
pub const ARGB_DEFAULT_LEDS: usize = 60;

pub mod mode {
    pub const OFF: u8 = 0;
    pub const STATIC: u8 = 1;
    pub const BREATHING: u8 = 2;
    pub const FLASHING: u8 = 3;
    pub const SPECTRUM_CYCLE: u8 = 4;
    pub const RAINBOW: u8 = 5;
    pub const DIRECT: u8 = 0xFF;
}

pub fn unified_to_mode(effect: EffectId) -> Option<u8> {
    Some(match effect {
        EffectId::Off => mode::OFF,
        EffectId::Static => mode::STATIC,
        EffectId::Breathing => mode::BREATHING,
        EffectId::Flashing => mode::FLASHING,
        EffectId::SpectrumCycle => mode::SPECTRUM_CYCLE,
        EffectId::RainbowWave => mode::RAINBOW,
        EffectId::ColorPulse => return None,
    })
}

/// LED layout decoded from the controller's config table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Onboard LEDs (excluding 12 V headers).
    pub onboard: u8,
    /// 12 V RGB headers; their slots follow the onboard LEDs.
    pub rgb_headers: u8,
    /// 5 V addressable headers.
    pub argb_headers: u8,
}

impl Layout {
    pub fn from_config_table(table: &[u8; 60]) -> Self {
        let total = table[0x1B];
        let rgb_headers = table[0x1D].min(total);
        let argb_headers = table[0x02].min(8);
        Layout {
            onboard: total - rgb_headers,
            rgb_headers,
            argb_headers,
        }
    }

    /// Mainboard effect-colour slots (onboard LEDs + RGB headers).
    pub fn fixed_slots(&self) -> usize {
        (self.onboard + self.rgb_headers) as usize
    }
}

fn packet(cmd: u8, body: &[u8]) -> [u8; REPORT_LEN] {
    let mut buf = [0u8; REPORT_LEN];
    buf[0] = REPORT_ID;
    buf[1] = cmd;
    buf[2..2 + body.len()].copy_from_slice(body);
    buf
}

pub fn request(cmd: u8) -> [u8; REPORT_LEN] {
    packet(cmd, &[])
}

pub fn effect_packet(channel: u8, mode: u8) -> [u8; REPORT_LEN] {
    packet(CMD_EFFECT, &[channel, 0x00, 0x00, mode])
}

/// Effect-mode colours for slots `0..colors.len()` (max 16).
pub fn effect_color_packet(colors: &[Rgb]) -> [u8; REPORT_LEN] {
    let n = colors.len().min(EFFECT_MAX_SLOTS);
    let mask: u16 = if n == 16 { 0xFFFF } else { (1u16 << n) - 1 };
    let mut body = vec![(mask >> 8) as u8, mask as u8, 0x00];
    body.extend(colors[..n].iter().flat_map(|c| [c.r, c.g, c.b]));
    packet(CMD_EFFECT_COLOR, &body)
}

/// Direct-mode packets for a channel, chunked to 20 LEDs; the last applies.
pub fn direct_packets(channel: u8, colors: &[Rgb]) -> Vec<[u8; REPORT_LEN]> {
    let chunks: Vec<_> = colors.chunks(DIRECT_MAX_LEDS).collect();
    chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let last = i + 1 == chunks.len();
            let mut body = vec![
                channel | if last { DIRECT_APPLY } else { 0 },
                (i * DIRECT_MAX_LEDS) as u8,
                chunk.len() as u8,
            ];
            body.extend(chunk.iter().flat_map(|c| [c.r, c.g, c.b]));
            packet(CMD_DIRECT, &body)
        })
        .collect()
}

struct Channel {
    device: Mutex<HidDevice>,
}

impl Channel {
    fn write_all(&self, packets: &[[u8; REPORT_LEN]]) -> RgbResult<()> {
        let dev = self.device.lock().map_err(|_| "HID channel poisoned")?;
        for p in packets {
            dev.write(p)
                .map_err(|e| format!("Aura USB write failed: {e}"))?;
        }
        Ok(())
    }

    fn query(&self, cmd: u8, reply: u8) -> RgbResult<[u8; REPORT_LEN]> {
        let dev = self.device.lock().map_err(|_| "HID channel poisoned")?;
        dev.write(&request(cmd))
            .map_err(|e| format!("Aura USB write failed: {e}"))?;
        let deadline = Instant::now() + Duration::from_millis(500);
        let mut buf = [0u8; REPORT_LEN];
        while !deadline.saturating_duration_since(Instant::now()).is_zero() {
            let n = dev
                .read_timeout(&mut buf, 100)
                .map_err(|e| format!("Aura USB read failed: {e}"))?;
            // Some backends strip the report id.
            let body = if n > 0 && buf[0] == REPORT_ID {
                &buf[1..n]
            } else {
                &buf[..n]
            };
            if body.first() == Some(&reply) {
                let mut out = [0u8; REPORT_LEN];
                out[1..1 + body.len()].copy_from_slice(body);
                out[0] = REPORT_ID;
                return Ok(out);
            }
        }
        Err(format!("no reply to Aura USB command 0x{cmd:02x}"))
    }
}

pub struct AuraUsbController {
    channel: Arc<Channel>,
    layout: Layout,
    fixed: Vec<Rgb>,
    argb: Vec<Rgb>,
    mode: u8,
    info: DeviceInfo,
}

impl AuraUsbController {
    async fn blocking<T: Send + 'static>(
        &self,
        f: impl FnOnce(&Channel) -> RgbResult<T> + Send + 'static,
    ) -> RgbResult<T> {
        let ch = self.channel.clone();
        tokio::task::spawn_blocking(move || f(&ch))
            .await
            .map_err(|e| format!("Aura USB worker panicked: {e}"))?
    }

    /// Static colours via direct mode: applies immediately, nothing written to flash.
    async fn apply_direct(&self) -> RgbResult<()> {
        let mut packets = vec![effect_packet(0, mode::DIRECT)];
        packets.extend(direct_packets(MAINBOARD_DIRECT_CHANNEL, &self.fixed));
        for (i, c) in self.argb.iter().enumerate() {
            packets.push(effect_packet(i as u8 + 1, mode::DIRECT));
            packets.extend(direct_packets(i as u8, &[*c; ARGB_DEFAULT_LEDS]));
        }
        self.blocking(move |ch| ch.write_all(&packets)).await
    }

    /// Hardware effect on every channel, with effect colours per slot.
    async fn apply_effect(&self, mode: u8) -> RgbResult<()> {
        let mut packets = vec![effect_packet(0, mode)];
        for i in 0..self.layout.argb_headers {
            packets.push(effect_packet(i + 1, mode));
        }
        let mut slots = self.fixed.clone();
        slots.extend(&self.argb);
        packets.push(effect_color_packet(&slots));
        self.blocking(move |ch| ch.write_all(&packets)).await
    }

    async fn apply(&self) -> RgbResult<()> {
        if self.mode == mode::STATIC {
            self.apply_direct().await
        } else {
            self.apply_effect(self.mode).await
        }
    }
}

#[async_trait]
impl RgbController for AuraUsbController {
    async fn initialize(&mut self) -> RgbResult<()> {
        let (firmware, table) = self
            .blocking(|ch| {
                let fw = ch.query(CMD_FIRMWARE, REPLY_FIRMWARE)?;
                let table = ch.query(CMD_CONFIG_TABLE, REPLY_CONFIG_TABLE)?;
                Ok((fw, table))
            })
            .await?;
        let fw = String::from_utf8_lossy(&firmware[2..18])
            .trim_end_matches('\0')
            .trim()
            .to_string();
        let mut cfg = [0u8; 60];
        cfg.copy_from_slice(&table[4..64]);
        let mut layout = Layout::from_config_table(&cfg);
        if layout.fixed_slots() == 0 && layout.argb_headers == 0 {
            return Err(format!("Aura controller {fw} reports no LEDs"));
        }
        // Keep everything addressable by the 16-bit effect-colour mask.
        let max_argb = EFFECT_MAX_SLOTS.saturating_sub(layout.fixed_slots()) as u8;
        layout.argb_headers = layout.argb_headers.min(max_argb);
        log::info!("Aura USB {fw}: {layout:?}");

        let mut zones = Vec::new();
        if layout.onboard > 0 {
            zones.push(("Mainboard".to_string(), layout.onboard as u16));
        }
        for i in 0..layout.rgb_headers {
            zones.push((format!("RGB Header {}", i + 1), 1));
        }
        for i in 0..layout.argb_headers {
            zones.push((format!("ARGB Header {}", i + 1), ARGB_DEFAULT_LEDS as u16));
        }
        self.info.zones = zones
            .into_iter()
            .enumerate()
            .map(|(i, (name, led_count))| ZoneInfo {
                index: i as u8,
                name,
                led_count,
            })
            .collect();
        self.info.firmware = Some(fw);
        self.info.effects = EffectId::ALL
            .into_iter()
            .filter(|e| unified_to_mode(*e).is_some())
            .map(EffectId::info)
            .collect();
        self.fixed = vec![Rgb::WHITE; layout.fixed_slots()];
        self.argb = vec![Rgb::WHITE; layout.argb_headers as usize];
        self.layout = layout;
        Ok(())
    }

    async fn set_color(&mut self, red: u8, green: u8, blue: u8) -> RgbResult<()> {
        let c = Rgb::new(red, green, blue);
        self.fixed.fill(c);
        self.argb.fill(c);
        if !matches!(self.mode, mode::BREATHING | mode::FLASHING) {
            self.mode = mode::STATIC;
        }
        self.apply().await
    }

    async fn set_effect(&mut self, effect_id: u8, _speed: u8) -> RgbResult<()> {
        // The USB controller has no speed register; animations run at the
        // firmware's fixed rate.
        let effect = EffectId::try_from(effect_id)?;
        self.mode = unified_to_mode(effect)
            .ok_or_else(|| format!("{} is not supported by Aura USB", effect.name()))?;
        self.apply().await
    }

    async fn get_device_info(&self) -> DeviceInfo {
        self.info.clone()
    }

    async fn set_zone_color(&mut self, zone: u8, color: Rgb) -> RgbResult<()> {
        let mut z = zone as usize;
        if self.layout.onboard > 0 {
            if z == 0 {
                self.fixed[..self.layout.onboard as usize].fill(color);
                return self.apply().await;
            }
            z -= 1;
        }
        let rgb = self.layout.rgb_headers as usize;
        if z < rgb {
            self.fixed[self.layout.onboard as usize + z] = color;
        } else if let Some(slot) = self.argb.get_mut(z - rgb) {
            *slot = color;
        } else {
            return Err(format!("zone {zone} out of range"));
        }
        self.apply().await
    }
}

/// Finds Aura USB mainboard controllers.
#[derive(Default)]
pub struct AuraUsbDetector {
    api: SharedHidApi,
}

impl AuraUsbDetector {
    pub fn new(api: SharedHidApi) -> Self {
        Self { api }
    }
}

fn detect_blocking(
    api: &mut HidApi,
    warnings: &mut Vec<String>,
    known: &HashSet<String>,
) -> Vec<Box<dyn RgbController>> {
    // Prefer the vendor collection (usage page 0xFF72); on Windows the
    // device also exposes unrelated collections under other paths.
    let mut by_pid: BTreeMap<(u16, String), (std::ffi::CString, String, bool)> = BTreeMap::new();
    for d in api.device_list() {
        if d.vendor_id() != ASUS_VID || !MAINBOARD_PIDS.contains(&d.product_id()) {
            continue;
        }
        let serial = d.serial_number().unwrap_or("").to_string();
        let vendor_page = d.usage_page() == AURA_USAGE_PAGE;
        let key = (d.product_id(), serial);
        let better = by_pid
            .get(&key)
            .map_or(true, |(_, _, had)| !had && vendor_page);
        if better {
            by_pid.insert(
                key,
                (
                    d.path().to_owned(),
                    d.path().to_string_lossy().into_owned(),
                    vendor_page,
                ),
            );
        }
    }

    let system = crate::system::SystemInfo::collect();
    let board = system
        .board_name
        .map(|b| format!("ASUS {b}"))
        .unwrap_or_else(|| "ASUS Aura Motherboard".into());

    let mut out: Vec<Box<dyn RgbController>> = Vec::new();
    for ((pid, serial), (path, path_display, _)) in by_pid {
        let key = if serial.is_empty() {
            path_display.clone()
        } else {
            serial.clone()
        };
        let id = format!("aura-usb:{pid:04x}:{key}");
        if known.contains(&id) {
            continue;
        }
        let device = match api.open_path(&path) {
            Ok(d) => d,
            Err(e) => {
                warnings.push(format!("cannot open Aura controller ({path_display}): {e}"));
                continue;
            }
        };
        out.push(Box::new(AuraUsbController {
            channel: Arc::new(Channel {
                device: Mutex::new(device),
            }),
            layout: Layout {
                onboard: 0,
                rgb_headers: 0,
                argb_headers: 0,
            },
            fixed: Vec::new(),
            argb: Vec::new(),
            mode: mode::STATIC,
            info: DeviceInfo {
                id,
                name: board.clone(),
                vendor: "ASUS".into(),
                device_type: DeviceType::Motherboard,
                transport: Transport::Usb,
                location: path_display,
                serial: (!serial.is_empty()).then_some(serial),
                firmware: None,
                zones: Vec::new(),
                effects: Vec::new(),
                layout: None,
            },
        }));
    }
    out
}

#[async_trait]
impl DeviceDetector for AuraUsbDetector {
    fn name(&self) -> &'static str {
        "ASUS Aura USB"
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
        let mut report = DetectionReport::default();
        match result {
            Ok(Ok((controllers, warnings))) => {
                report.controllers = controllers;
                report.warnings = warnings;
            }
            Ok(Err(e)) => report.warn(e),
            Err(e) => report.warn(format!("Aura USB detection panicked: {e}")),
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_packet_layout() {
        let p = effect_packet(2, mode::BREATHING);
        assert_eq!(&p[..6], &[0xEC, 0x35, 0x02, 0x00, 0x00, 0x02]);
        assert_eq!(p.len(), 65);
    }

    #[test]
    fn effect_color_mask_and_rgb_order() {
        let p = effect_color_packet(&[Rgb::new(1, 2, 3), Rgb::new(4, 5, 6), Rgb::new(7, 8, 9)]);
        assert_eq!(
            &p[..14],
            &[0xEC, 0x36, 0x00, 0x07, 0x00, 1, 2, 3, 4, 5, 6, 7, 8, 9]
        );
        let full = effect_color_packet(&[Rgb::WHITE; 20]);
        assert_eq!(&full[2..4], &[0xFF, 0xFF]);
        assert_eq!(full[5 + 16 * 3], 0, "slots beyond 16 are dropped");
    }

    #[test]
    fn direct_packets_chunk_and_apply_last() {
        let ps = direct_packets(1, &[Rgb::new(9, 8, 7); 45]);
        assert_eq!(ps.len(), 3);
        assert_eq!(&ps[0][..5], &[0xEC, 0x40, 0x01, 0, 20]);
        assert_eq!(&ps[1][..5], &[0xEC, 0x40, 0x01, 20, 20]);
        assert_eq!(&ps[2][..5], &[0xEC, 0x40, 0x81, 40, 5]);
        assert_eq!(&ps[2][5..8], &[9, 8, 7]);
    }

    #[test]
    fn layout_from_config_table() {
        let mut t = [0u8; 60];
        t[0x02] = 2; // ARGB headers
        t[0x1B] = 6; // mainboard LEDs incl. RGB headers
        t[0x1D] = 2; // RGB headers
        let l = Layout::from_config_table(&t);
        assert_eq!(
            l,
            Layout {
                onboard: 4,
                rgb_headers: 2,
                argb_headers: 2
            }
        );
        assert_eq!(l.fixed_slots(), 6);
    }
}
