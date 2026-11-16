import type { Backend } from "./backend";
import type { ActiveProfile, Device, DeviceType, EffectInfo, Lang, ManagerEvent, Profile, Rgb, ZoneInfo } from "./types";
import { EffectIds } from "./types";

const EFFECT_NAMES: Record<number, [string, boolean, boolean]> = {
  0: ["Off", false, false],
  1: ["Static", true, false],
  2: ["Breathing", true, true],
  3: ["Spectrum Cycle", false, true],
  4: ["Rainbow Wave", false, true],
  5: ["Color Pulse", true, true],
  6: ["Flashing", true, true],
};

function effects(ids: number[]): EffectInfo[] {
  return ids.map((id) => {
    const [name, uses_color, uses_speed] = EFFECT_NAMES[id]!;
    return { id, name, uses_color, uses_speed };
  });
}

function zones(list: [string, number][]): ZoneInfo[] {
  return list.map(([name, led_count], index) => ({ index, name, led_count }));
}

function device(
  slug: string,
  name: string,
  vendor: string,
  device_type: DeviceType,
  z: [string, number][],
  e: number[],
  transport: Device["transport"] = "virtual",
): Device {
  return {
    id: `virtual:${slug}`,
    name,
    vendor,
    device_type,
    transport,
    location: "simulated (browser demo)",
    serial: `SIM-${slug}`,
    firmware: "sim-1.0",
    zones: zones(z),
    effects: effects(e),
    status: { state: "ready" },
    color: null,
    effect: null,
    speed: 50,
    battery: null,
    wireless: transport === "ble",
    layout: null,
    keys: {},
  };
}

function withLayout(d: Device, layout: string): Device {
  return { ...d, layout };
}

function withBattery(d: Device, percent: number, charging: boolean): Device {
  return { ...d, battery: { percent, charging, approximate: false }, wireless: true };
}

const { Off, Static, Breathing, SpectrumCycle, RainbowWave, ColorPulse, Flashing } = EffectIds;

function demoDevices(): Device[] {
  return [
    device("aura-mobo", "Virtual ASUS Aura Motherboard", "ASUS", "motherboard",
      [["Onboard", 6], ["ARGB Header 1", 60], ["ARGB Header 2", 30]],
      [Off, Static, Breathing, Flashing, SpectrumCycle, RainbowWave]),
    withLayout(device("g-keyboard", "Virtual Logitech G Keyboard", "Logitech", "keyboard",
      [["Primary", 1], ["Logo", 1]], [Off, Static, Breathing, SpectrumCycle, ColorPulse]), "logitech-g910"),
    device("g-mouse", "Virtual Logitech G Mouse", "Logitech", "mouse",
      [["Primary", 1], ["Logo", 1]], [Off, Static, Breathing, SpectrumCycle]),
    withBattery(device("g-headset", "Virtual Logitech G Headset", "Logitech", "headset",
      [["Logo", 1], ["Strip", 1]], [Off, Static, Breathing, SpectrumCycle]), 64, false),
    device("g-speakers", "Virtual Logitech G Speakers", "Logitech", "speaker",
      [["Left", 1], ["Right", 1], ["Left Rear", 1], ["Right Rear", 1]],
      [Off, Static, Breathing, SpectrumCycle]),
    withBattery(device("ble-strip", "Virtual BLE LED Strip", "ESP32", "led_strip",
      [["Strip", 120]], [Off, Static, Breathing, SpectrumCycle, RainbowWave, Flashing], "ble"), 87, true),
  ];
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Browser-only simulator mirroring the Rust manager's event semantics. */
export function createDemoBackend(): Backend {
  const devices = new Map<string, Device>();
  const listeners = new Set<(e: ManagerEvent) => void>();
  let enabled = true;
  let scanning = false;
  let language: Lang = "sk";
  let autostart = false;
  const profiles: Profile[] = [];
  let defaultProfile: string | null = null;
  let autoSwitch = true;
  let active: ActiveProfile = { id: null, game: null };
  const profileListeners = new Set<(a: ActiveProfile) => void>();
  const settingsListeners = new Set<(s: { language: Lang }) => void>();
  const emit = (e: ManagerEvent) => listeners.forEach((l) => l(structuredClone(e)));

  const update = (id: string, fn: (d: Device) => void) => {
    const d = devices.get(id);
    if (!d) throw `unknown device '${id}'`;
    fn(d);
    d.status = { state: "ready" };
    emit({ type: "device_updated", device: d });
  };

  const backend: Backend = {
    kind: "demo",
    window: null,
    async listDevices() {
      return structuredClone([...devices.values()]);
    },
    async isScanning() {
      return scanning;
    },
    async scan() {
      if (scanning) throw "a scan is already in progress";
      scanning = true;
      const started = performance.now();
      emit({ type: "scan_started" });
      let added = 0;
      if (enabled) {
        for (const d of demoDevices()) {
          await sleep(120);
          if (!devices.has(d.id)) {
            devices.set(d.id, d);
            added++;
            emit({ type: "device_added", device: d });
          }
        }
      }
      const warnings = [
        language === "sk"
          ? "V prehliadači nie je prístup k hardvéru; spustite `npm run tauri dev`."
          : "Hardware access is unavailable in the browser demo; run `npm run tauri dev`.",
      ];
      emit({ type: "warning", source: "Demo", message: warnings[0]! });
      const summary = {
        added,
        total: devices.size,
        duration_ms: Math.round(performance.now() - started),
        warnings,
      };
      scanning = false;
      emit({ type: "scan_finished", ...summary });
      return summary;
    },
    async setColor(id, color: Rgb) {
      await sleep(8);
      update(id, (d) => {
        d.keys = {};
        d.color = color;
        const keeps = d.effects.find((e) => e.id === d.effect)?.uses_color ?? false;
        if (!keeps) d.effect = Static;
      });
    },
    async setColorMany(ids, color) {
      await Promise.all(ids.map((id) => backend.setColor(id, color)));
      return [];
    },
    async setZoneColor(id, _zone, color) {
      await sleep(8);
      update(id, (d) => {
        if (d.zones.length <= 1) d.color = color;
      });
    },
    async setEffect(id, effectId, speed) {
      await sleep(8);
      const d = devices.get(id);
      if (d && !d.effects.some((e) => e.id === effectId)) {
        const message = `${d.name} does not support ${EFFECT_NAMES[effectId]?.[0] ?? effectId}`;
        d.status = { state: "error", message };
        emit({ type: "device_error", id, message });
        emit({ type: "device_updated", device: d });
        throw message;
      }
      update(id, (dev) => {
        dev.effect = effectId;
        dev.speed = Math.min(speed, 100);
      });
    },
    async getSystemInfo() {
      return {
        os: navigator.platform || "browser",
        arch: "browser",
        board_vendor: "ASUSTeK COMPUTER INC. (simulated)",
        board_name: "ROG STRIX Z790-E (simulated)",
        system_product: null,
        bios_vendor: "American Megatrends Inc.",
        bios_version: "demo",
        cpu: null,
      };
    },
    async getSettings() {
      return { virtual_devices: enabled, language, custom_tray_menu: false };
    },
    async setLanguage(lang) {
      language = lang;
      settingsListeners.forEach((l) => l({ language }));
    },
    async getAutostart() {
      return autostart;
    },
    async setAutostart(on) {
      autostart = on;
    },
    async showMainWindow() {},
    async hideTrayMenu() {},
    async resizeTrayMenu() {},
    async quit() {},
    async onSettings(cb) {
      settingsListeners.add(cb);
      return () => settingsListeners.delete(cb);
    },
    async onTrayOpened() {
      return () => {};
    },
    async setKeyColors(id, keys) {
      await sleep(8);
      const d = devices.get(id);
      if (!d?.layout) throw "per-key lighting is not supported by this device";
      update(id, (dev) => {
        dev.keys = { ...dev.keys, ...Object.fromEntries(keys) };
      });
    },
    async setTrayTooltip() {},
    async getProfiles() {
      return structuredClone({ profiles, default_profile: defaultProfile, auto_switch: autoSwitch, active });
    },
    async saveProfile(p) {
      const i = profiles.findIndex((x) => x.id === p.id);
      if (i >= 0) profiles[i] = structuredClone(p);
      else profiles.push(structuredClone(p));
    },
    async deleteProfile(id) {
      const i = profiles.findIndex((x) => x.id === id);
      if (i >= 0) profiles.splice(i, 1);
      if (defaultProfile === id) defaultProfile = null;
    },
    async applyProfile(id) {
      const p = profiles.find((x) => x.id === id);
      if (!p) throw `unknown profile '${id}'`;
      if (p.color) await backend.setColorMany([...devices.keys()], p.color);
      for (const d of devices.values()) {
        if (d.layout && Object.keys(p.keys).length) await backend.setKeyColors(d.id, Object.entries(p.keys));
      }
      active = { id, game: null };
      profileListeners.forEach((l) => l(active));
    },
    async setDefaultProfile(id) {
      defaultProfile = id;
    },
    async setAutoSwitch(on) {
      autoSwitch = on;
    },
    async runningProcesses() {
      return ["chrome.exe", "cs2.exe", "discord.exe", "explorer.exe", "steam.exe"];
    },
    async onProfile(cb) {
      profileListeners.add(cb);
      return () => profileListeners.delete(cb);
    },
    async getDeviceDatabase() {
      return (await import("./demoDatabase.json")).default as never;
    },
    async setVirtualDevices(on) {
      enabled = on;
      if (on) {
        await backend.scan();
      } else {
        for (const id of [...devices.keys()]) {
          devices.delete(id);
          emit({ type: "device_removed", id });
        }
      }
    },
    async onEvent(cb) {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    async onResync() {
      return () => {};
    },
  };
  return backend;
}
