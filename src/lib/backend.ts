import type { Device, DeviceFailure, Lang, ManagerEvent, Rgb, ScanSummary, Settings, SystemInfo } from "./types";

export interface Backend {
  readonly kind: "tauri" | "demo";
  listDevices(): Promise<Device[]>;
  scan(): Promise<ScanSummary>;
  isScanning(): Promise<boolean>;
  setColor(deviceId: string, color: Rgb): Promise<void>;
  setColorMany(deviceIds: string[], color: Rgb): Promise<DeviceFailure[]>;
  setZoneColor(deviceId: string, zone: number, color: Rgb): Promise<void>;
  setEffect(deviceId: string, effectId: number, speed: number): Promise<void>;
  getSystemInfo(): Promise<SystemInfo>;
  getSettings(): Promise<Settings>;
  setVirtualDevices(enabled: boolean): Promise<void>;
  setLanguage(lang: Lang): Promise<void>;
  getAutostart(): Promise<boolean>;
  setAutostart(enabled: boolean): Promise<void>;
  showMainWindow(): Promise<void>;
  hideTrayMenu(): Promise<void>;
  resizeTrayMenu(height: number): Promise<void>;
  quit(): Promise<void>;
  /** Persisted settings changed (in this or another window). */
  onSettings(cb: (s: { language: Lang }) => void): Promise<() => void>;
  /** The tray menu window was opened. */
  onTrayOpened(cb: () => void): Promise<() => void>;
  /** Subscribe to backend events. Resolves to an unsubscribe function. */
  onEvent(cb: (e: ManagerEvent) => void): Promise<() => void>;
  /** Called when events may have been dropped and state should be refetched. */
  onResync(cb: () => void): Promise<() => void>;
}

const EVENT_CHANNEL = "rgb://event";
const RESYNC_CHANNEL = "rgb://resync";
const SETTINGS_CHANNEL = "rgb://settings";
const TRAY_OPENED = "tray-menu://opened";

async function createTauriBackend(): Promise<Backend> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  return {
    kind: "tauri",
    listDevices: () => invoke("list_devices"),
    scan: () => invoke("scan_devices"),
    isScanning: () => invoke("is_scanning"),
    setColor: (deviceId, color) => invoke("set_color", { deviceId, color }),
    setColorMany: (deviceIds, color) => invoke("set_color_many", { deviceIds, color }),
    setZoneColor: (deviceId, zone, color) => invoke("set_zone_color", { deviceId, zone, color }),
    setEffect: (deviceId, effectId, speed) => invoke("set_effect", { deviceId, effectId, speed }),
    getSystemInfo: () => invoke("get_system_info"),
    getSettings: () => invoke("get_settings"),
    setVirtualDevices: (enabled) => invoke("set_virtual_devices", { enabled }),
    setLanguage: (language) => invoke("set_language", { language }),
    getAutostart: () => invoke("get_autostart"),
    setAutostart: (enabled) => invoke("set_autostart", { enabled }),
    showMainWindow: () => invoke("show_main_window"),
    hideTrayMenu: () => invoke("hide_tray_menu"),
    resizeTrayMenu: (height) => invoke("tray_menu_resize", { height }),
    quit: () => invoke("quit_app"),
    onSettings: (cb) => listen<{ language: Lang }>(SETTINGS_CHANNEL, (e) => cb(e.payload)),
    onTrayOpened: (cb) => listen(TRAY_OPENED, () => cb()),
    onEvent: (cb) => listen<ManagerEvent>(EVENT_CHANNEL, (e) => cb(e.payload)),
    onResync: (cb) => listen(RESYNC_CHANNEL, () => cb()),
  };
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

let backendPromise: Promise<Backend> | null = null;

/**
 * Inside the Tauri webview this talks to Rust. In a plain browser
 * (`npm run dev`) it falls back to an in-memory simulator so the UI can be
 * developed without hardware or a Rust toolchain.
 */
export function getBackend(): Promise<Backend> {
  backendPromise ??= isTauri()
    ? createTauriBackend()
    : import("./demoBackend").then((m) => m.createDemoBackend());
  return backendPromise;
}

export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return JSON.stringify(e);
}
