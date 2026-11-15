// Mirrors the serde output of `rgb-core` (snake_case on purpose).

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

export type DeviceType =
  | "motherboard"
  | "dram"
  | "keyboard"
  | "mouse"
  | "speaker"
  | "headset"
  | "led_strip"
  | "light"
  | "unknown";

export type Transport = "usb" | "smbus" | "ble" | "virtual";

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

export interface Settings {
  virtual_devices: boolean;
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
