// Mirrors the serde output of `rgb-core` (snake_case on purpose).

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

export type DeviceType =
  | "motherboard"
  | "gpu"
  | "dram"
  | "keyboard"
  | "mouse"
  | "speaker"
  | "headset"
  | "led_strip"
  | "light"
  | "unknown";

export type Transport = "usb" | "smbus" | "i2c" | "ble" | "virtual";

export interface ZoneInfo {
  index: number;
  name: string;
  led_count: number;
}

export interface EffectInfo {
  id: number;
  name: string;
  uses_color: boolean;
  uses_speed: boolean;
}

export interface BatteryInfo {
  percent: number;
  charging: boolean;
  /** Estimated from voltage rather than reported by the device. */
  approximate: boolean;
}

export type DeviceStatus = { state: "ready" } | { state: "error"; message: string };

export interface Device {
  id: string;
  name: string;
  vendor: string;
  device_type: DeviceType;
  transport: Transport;
  location: string;
  serial: string | null;
  firmware: string | null;
  zones: ZoneInfo[];
  effects: EffectInfo[];
  status: DeviceStatus;
  color: Rgb | null;
  effect: number | null;
  speed: number;
  battery: BatteryInfo | null;
  wireless: boolean;
  /** Per-key layout id (e.g. "logitech-g910") when keys can be lit individually. */
  layout: string | null;
  /** Per-key colours last applied, by key id (`k:XX`, `g:N`, `logo`). */
  keys: Record<string, Rgb>;
}

export interface ScanSummary {
  added: number;
  total: number;
  duration_ms: number;
  warnings: string[];
}

export type ManagerEvent =
  | { type: "scan_started" }
  | ({ type: "scan_finished" } & ScanSummary)
  | { type: "device_added"; device: Device }
  | { type: "device_updated"; device: Device }
  | { type: "device_removed"; id: string }
  | { type: "device_error"; id: string; message: string }
  | { type: "warning"; source: string; message: string };

export interface SystemInfo {
  os: string;
  arch: string;
  board_vendor: string | null;
  board_name: string | null;
  system_product: string | null;
  bios_vendor: string | null;
  bios_version: string | null;
  cpu: string | null;
}

export const LANGS = [
  "sk", "en", "cs", "hu", "es-ES", "es-419", "zh-Hans", "zh-Hant", "de", "fr", "ja", "pt-BR", "pt-PT",
  "ko", "ar", "hi", "it", "id", "vi", "th", "tr", "pl", "uk", "nl", "sv",
] as const;
export type Lang = (typeof LANGS)[number];

export interface Settings {
  virtual_devices: boolean;
  language: Lang;
  /** Windows/macOS use the app-styled tray menu window. */
  custom_tray_menu: boolean;
  /** Where the log file is written (desktop app only). */
  log_file?: string | null;
}

export interface DeviceFailure {
  id: string;
  message: string;
}

/** Keep in sync with `EffectId` in crates/rgb-core/src/types.rs. */
export const EffectIds = {
  Off: 0,
  Static: 1,
  Breathing: 2,
  SpectrumCycle: 3,
  RainbowWave: 4,
  ColorPulse: 5,
  Flashing: 6,
} as const;

export interface Profile {
  id: string;
  name: string;
  /** Executable names that activate this profile automatically. */
  games: string[];
  /** Colour applied to every device first. */
  color: Rgb | null;
  /** Per-key colours for keyboards with a layout. */
  keys: Record<string, Rgb>;
}

export interface ActiveProfile {
  id: string | null;
  game: string | null;
}

export interface ProfilesView {
  profiles: Profile[];
  default_profile: string | null;
  auto_switch: boolean;
  active: ActiveProfile;
}

export type Support = "verified" | "supported" | "experimental" | "planned";

export interface KnownDevice {
  vendor: string;
  name: string;
  kind: DeviceType;
  bus: "usb" | "pci" | "smbus" | "ble";
  ids: string;
  match_key: string;
  support: Support;
  note: string | null;
}
