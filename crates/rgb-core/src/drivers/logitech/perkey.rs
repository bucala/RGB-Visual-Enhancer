//! Per-key lighting for Logitech G910 keyboards (Orion Spark / Spectrum).
//!
//! Packet layouts are the ones documented by the g810-led project for the
//! G910 (facts only; this is an independent implementation):
//!
//! ```text
//! keys   (64 B): 12 ff 0f 3d 00 01 00 0e  [code R G B] × 14
//! G-keys (64 B): 12 ff 0f 3e 00 04 00 09  [code R G B] × ≤9
//! logo   (20 B): 11 ff 0f 3a 00 10 00 02  [code R G B] × ≤2
//! commit (20 B): 11 ff 0f 5d
//! ```
//!
//! Main-key codes are standard HID keyboard usage ids (A = 0x04, …).
//!
//! Key ids used across the app: `k:XX` (hex HID usage), `g:N` (G1–G9),
//! `logo`.

use crate::types::Rgb;

pub const G910_PIDS: [u16; 2] = [0xC32B, 0xC335];
pub const LAYOUT_G910: &str = "logitech-g910";

const KEYS_HEADER: [u8; 8] = [0x12, 0xFF, 0x0F, 0x3D, 0x00, 0x01, 0x00, 0x0E];
const GKEYS_HEADER: [u8; 8] = [0x12, 0xFF, 0x0F, 0x3E, 0x00, 0x04, 0x00, 0x09];
const LOGO_HEADER: [u8; 8] = [0x11, 0xFF, 0x0F, 0x3A, 0x00, 0x10, 0x00, 0x02];
const COMMIT: [u8; 4] = [0x11, 0xFF, 0x0F, 0x5D];

const KEYS_PER_PACKET: usize = 14;
const MAX_GKEYS: usize = 9;
const MAX_LOGO: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    /// Main key by HID usage id.
    Main(u8),
    /// G-key 1–9.
    G(u8),
    Logo,
}

impl Key {
    pub fn parse(id: &str) -> Option<Key> {
        if id == "logo" {
            return Some(Key::Logo);
        }
        if let Some(n) = id.strip_prefix("g:") {
            let n: u8 = n.parse().ok()?;
            return (1..=9).contains(&n).then_some(Key::G(n));
        }
        let code = u8::from_str_radix(id.strip_prefix("k:")?, 16).ok()?;
        valid_main_code(code).then_some(Key::Main(code))
    }

    pub fn id(self) -> String {
        match self {
            Key::Main(c) => format!("k:{c:02x}"),
            Key::G(n) => format!("g:{n}"),
            Key::Logo => "logo".into(),
        }
    }
}

/// Codes the G910 accepts in the main-key group.
pub fn valid_main_code(code: u8) -> bool {
    matches!(code, 0x04..=0x65 | 0x87 | 0xE0..=0xE7)
}

fn packet(len: usize, header: &[u8], entries: &[(u8, Rgb)]) -> Vec<u8> {
    let mut p = vec![0u8; len];
    p[..header.len()].copy_from_slice(header);
    for (i, (code, c)) in entries.iter().enumerate() {
        let o = header.len() + i * 4;
        p[o..o + 4].copy_from_slice(&[*code, c.r, c.g, c.b]);
    }
    p
}

/// Reports to send (in order) to apply `colors`, ending with the commit.
/// Unknown/invalid keys are ignored.
pub fn g910_reports(colors: &[(Key, Rgb)]) -> Vec<Vec<u8>> {
    let mut main = Vec::new();
    let mut gkeys = Vec::new();
    let mut logo = Vec::new();
    for (key, c) in colors {
        match *key {
            Key::Main(code) if valid_main_code(code) => main.push((code, *c)),
            Key::G(n) if (1..=9).contains(&n) => gkeys.push((n, *c)),
            Key::Logo => logo.push((0x01, *c)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for chunk in main.chunks(KEYS_PER_PACKET) {
        out.push(packet(64, &KEYS_HEADER, chunk));
    }
    if !gkeys.is_empty() {
        gkeys.truncate(MAX_GKEYS);
        out.push(packet(64, &GKEYS_HEADER, &gkeys));
    }
    if !logo.is_empty() {
        logo.truncate(MAX_LOGO);
        out.push(packet(20, &LOGO_HEADER, &logo));
    }
    if !out.is_empty() {
        out.push(packet(20, &COMMIT, &[]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_key_ids() {
        assert_eq!(Key::parse("k:04"), Some(Key::Main(0x04)));
        assert_eq!(Key::parse("k:e0"), Some(Key::Main(0xE0)));
        assert_eq!(Key::parse("g:9"), Some(Key::G(9)));
        assert_eq!(Key::parse("logo"), Some(Key::Logo));
        assert_eq!(Key::parse("g:10"), None);
        assert_eq!(Key::parse("k:02"), None, "not a key usage");
        assert_eq!(Key::parse("k:zz"), None);
        assert_eq!(Key::Main(0x1A).id(), "k:1a");
    }

    #[test]
    fn main_keys_are_chunked_by_14_then_committed() {
        let colors: Vec<_> = (0x04..0x04 + 20)
            .map(|c| (Key::Main(c), Rgb::new(1, 2, 3)))
            .collect();
        let r = g910_reports(&colors);
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].len(), 64);
        assert_eq!(&r[0][..8], &KEYS_HEADER);
        assert_eq!(&r[0][8..12], &[0x04, 1, 2, 3]);
        assert_eq!(&r[0][8 + 13 * 4..8 + 14 * 4], &[0x11, 1, 2, 3]);
        assert_eq!(&r[1][8..12], &[0x12, 1, 2, 3]);
        assert_eq!(r[1][8 + 6 * 4], 0, "unused slots are zero");
        assert_eq!(r[2].len(), 20);
        assert_eq!(&r[2][..4], &COMMIT);
    }

    #[test]
    fn gkeys_and_logo_use_their_groups() {
        let r = g910_reports(&[(Key::G(3), Rgb::new(9, 8, 7)), (Key::Logo, Rgb::WHITE)]);
        assert_eq!(&r[0][..8], &GKEYS_HEADER);
        assert_eq!(&r[0][8..12], &[3, 9, 8, 7]);
        assert_eq!(r[1].len(), 20);
        assert_eq!(
            &r[1][..12],
            &[0x11, 0xFF, 0x0F, 0x3A, 0x00, 0x10, 0x00, 0x02, 0x01, 255, 255, 255]
        );
        assert_eq!(&r[2][..4], &COMMIT);
    }

    #[test]
    fn nothing_to_send_means_no_commit() {
        assert!(g910_reports(&[]).is_empty());
        assert!(g910_reports(&[(Key::Main(0x01), Rgb::WHITE)]).is_empty());
    }
}
