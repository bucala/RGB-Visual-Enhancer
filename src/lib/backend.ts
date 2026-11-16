import type {
  ActiveProfile,
  Device,
  DeviceFailure,
  KnownDevice,
  Lang,
  ManagerEvent,
  Profile,
  ProfilesView,
  Rgb,
  ScanSummary,
  Settings,
  SystemInfo,
} from "./types";

/** Controls for the frameless main window (absent in the browser demo). */
export interface WindowControls {
  minimize(): Promise<void>;
  toggleMaximize(): Promise<void>;
  close(): Promise<void>;
  isMaximized(): Promise<boolean>;
  onResized(cb: () => void): Promise<() => void>;
}

export interface Backend {
  readonly kind: "tauri" | "demo";
  readonly window: WindowControls | null;
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
  setKeyColors(deviceId: string, keys: [string, Rgb][]): Promise<void>;
  setTrayTooltip(text: string): Promise<void>;
  getProfiles(): Promise<ProfilesView>;
  saveProfile(profile: Profile): Promise<void>;
  deleteProfile(id: string): Promise<void>;
  applyProfile(id: string): Promise<void>;
  setDefaultProfile(id: string | null): Promise<void>;
  setAutoSwitch(enabled: boolean): Promise<void>;
  runningProcesses(): Promise<string[]>;
  onProfile(cb: (active: ActiveProfile) => void): Promise<() => void>;
  getDeviceDatabase(): Promise<KnownDevice[]>;
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
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const win = getCurrentWindow();
  return {
    kind: "tauri",
    window: {
      minimize: () => win.minimize(),
      toggleMaximize: () => win.toggleMaximize(),
      // Closing hides to the tray when one is available (handled in Rust).
      close: () => win.close(),
      isMaximized: () => win.isMaximized(),
      onResized: (cb) => win.onResized(() => cb()),
    },
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
    setKeyColors: (deviceId, keys) => invoke("set_key_colors", { deviceId, keys }),
    setTrayTooltip: (text) => invoke("set_tray_tooltip", { text }),
    getProfiles: () => invoke("get_profiles"),
    saveProfile: (profile) => invoke("save_profile", { profile }),
    deleteProfile: (id) => invoke("delete_profile", { id }),
    applyProfile: (id) => invoke("apply_profile", { id }),
    setDefaultProfile: (id) => invoke("set_default_profile", { id }),
    setAutoSwitch: (enabled) => invoke("set_auto_switch", { enabled }),
    runningProcesses: () => invoke("running_processes"),
    onProfile: (cb) => listen<ActiveProfile>("rgb://profile", (e) => cb(e.payload)),
    getDeviceDatabase: () => invoke("get_device_database"),
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
