import { useCallback, useEffect, useMemo, useReducer, useRef } from "react";
import { errorMessage, getBackend, type Backend } from "../lib/backend";
import { createLatestSender } from "../lib/latestSender";
import type { Device, ManagerEvent, Rgb, ScanSummary, Settings, SystemInfo } from "../lib/types";

export interface LogEntry {
  id: number;
  time: number;
  level: "info" | "warn" | "error";
  source: string;
  message: string;
}

interface State {
  backendKind: Backend["kind"] | null;
  devices: Record<string, Device>;
  selectedId: string | null;
  scanning: boolean;
  lastScan: ScanSummary | null;
  system: SystemInfo | null;
  settings: Settings;
  syncAll: boolean;
  log: LogEntry[];
}

type Action =
  | { type: "init"; kind: Backend["kind"]; devices: Device[]; settings: Settings; scanning: boolean }
  | { type: "system"; system: SystemInfo }
  | { type: "event"; event: ManagerEvent }
  | { type: "replace"; devices: Device[] }
  | { type: "select"; id: string | null }
  | { type: "sync"; on: boolean }
  | { type: "settings"; settings: Settings }
  | { type: "log"; level: LogEntry["level"]; source: string; message: string };

const MAX_LOG = 200;
let logSeq = 0;

function pushLog(state: State, level: LogEntry["level"], source: string, message: string): LogEntry[] {
  const entry = { id: ++logSeq, time: Date.now(), level, source, message };
  return [entry, ...state.log].slice(0, MAX_LOG);
}

function byId(list: Device[]): Record<string, Device> {
  return Object.fromEntries(list.map((d) => [d.id, d]));
}

function firstId(devices: Record<string, Device>): string | null {
  return Object.keys(devices)[0] ?? null;
}

function reducer(state: State, action: Action): State {
  switch (action.type) {
    case "init": {
      const devices = byId(action.devices);
      return {
        ...state,
        backendKind: action.kind,
        devices,
        settings: action.settings,
        scanning: action.scanning,
        selectedId: state.selectedId ?? firstId(devices),
      };
    }
    case "system":
      return { ...state, system: action.system };
    case "replace": {
      const devices = byId(action.devices);
      const selectedId = state.selectedId && devices[state.selectedId] ? state.selectedId : firstId(devices);
      return { ...state, devices, selectedId };
    }
    case "select":
      return { ...state, selectedId: action.id };
    case "sync":
      return { ...state, syncAll: action.on };
    case "settings":
      return { ...state, settings: action.settings };
    case "log":
      return { ...state, log: pushLog(state, action.level, action.source, action.message) };
    case "event":
      return applyEvent(state, action.event);
  }
}

function applyEvent(state: State, e: ManagerEvent): State {
  switch (e.type) {
    case "scan_started":
      return { ...state, scanning: true, log: pushLog(state, "info", "Scan", "Scanning for devices…") };
    case "scan_finished": {
      const { type: _type, ...summary } = e;
      const msg = `Found ${summary.added} new device(s), ${summary.total} total in ${(summary.duration_ms / 1000).toFixed(1)} s`;
      return { ...state, scanning: false, lastScan: summary, log: pushLog(state, "info", "Scan", msg) };
    }
    case "device_added": {
      const devices = { ...state.devices, [e.device.id]: e.device };
      return {
        ...state,
        devices,
        selectedId: state.selectedId ?? e.device.id,
        log: pushLog(state, "info", e.device.vendor, `Connected ${e.device.name}`),
      };
    }
    case "device_updated":
      return { ...state, devices: { ...state.devices, [e.device.id]: e.device } };
    case "device_removed": {
      const devices = { ...state.devices };
      delete devices[e.id];
      const selectedId = state.selectedId === e.id ? firstId(devices) : state.selectedId;
      return { ...state, devices, selectedId };
    }
    case "device_error": {
      const name = state.devices[e.id]?.name ?? e.id;
      return { ...state, log: pushLog(state, "error", name, e.message) };
    }
    case "warning":
      return { ...state, log: pushLog(state, "warn", e.source, e.message) };
  }
}

const initialState: State = {
  backendKind: null,
  devices: {},
  selectedId: null,
  scanning: false,
  lastScan: null,
  system: null,
  settings: { virtual_devices: false },
  syncAll: false,
  log: [],
};

export function useRgb() {
  const [state, dispatch] = useReducer(reducer, initialState);
  const backendRef = useRef<Backend | null>(null);

  const logError = useCallback(
    (source: string, e: unknown) => dispatch({ type: "log", level: "error", source, message: errorMessage(e) }),
    [],
  );

  // Connect to the backend, load state, subscribe to events.
  useEffect(() => {
    let disposed = false;
    const unsubs: (() => void)[] = [];
    (async () => {
      const backend = await getBackend();
      if (disposed) return;
      backendRef.current = backend;
      unsubs.push(await backend.onEvent((event) => dispatch({ type: "event", event })));
      unsubs.push(
        await backend.onResync(async () => dispatch({ type: "replace", devices: await backend.listDevices() })),
      );
      const [devices, settings, scanning] = await Promise.all([
        backend.listDevices(),
        backend.getSettings(),
        backend.isScanning(),
      ]);
      if (disposed) return;
      dispatch({ type: "init", kind: backend.kind, devices, settings, scanning });
      backend.getSystemInfo().then((system) => !disposed && dispatch({ type: "system", system }), (e) => logError("System", e));
      // The Rust side scans at startup; the browser demo needs a nudge.
      if (backend.kind === "demo" && devices.length === 0 && !scanning) {
        backend.scan().catch((e) => logError("Scan", e));
      }
    })().catch((e) => logError("Backend", e));
    return () => {
      disposed = true;
      unsubs.forEach((u) => u());
    };
  }, [logError]);

  // Latest-wins senders keep slider/picker drags responsive.
  const senders = useMemo(() => {
    const be = () => {
      if (!backendRef.current) throw new Error("backend not ready");
      return backendRef.current;
    };
    return {
      color: createLatestSender<Rgb>((id, c) => be().setColor(id, c), () => {}),
      many: createLatestSender<{ ids: string[]; color: Rgb }>(async (_k, v) => {
        const failures = await be().setColorMany(v.ids, v.color);
        failures.forEach((f) => logError(f.id, f.message));
      }, (_k, e) => logError("Sync", e)),
      effect: createLatestSender<{ effect: number; speed: number }>(
        (id, v) => be().setEffect(id, v.effect, v.speed),
        () => {},
      ),
    };
  }, [logError]);

  const devices = useMemo(() => Object.values(state.devices), [state.devices]);
  const selected = state.selectedId ? (state.devices[state.selectedId] ?? null) : null;

  const actions = useMemo(
    () => ({
      scan: () => backendRef.current?.scan().catch((e) => logError("Scan", e)),
      select: (id: string) => dispatch({ type: "select", id }),
      setSyncAll: (on: boolean) => dispatch({ type: "sync", on }),
      setColor: (deviceId: string, color: Rgb, syncAll: boolean, allIds: string[]) => {
        if (syncAll) senders.many("all", { ids: allIds, color });
        else senders.color(deviceId, color);
      },
      setZoneColor: (deviceId: string, zone: number, color: Rgb) =>
        backendRef.current?.setZoneColor(deviceId, zone, color).catch(() => {}),
      setEffect: (deviceId: string, effect: number, speed: number) => senders.effect(deviceId, { effect, speed }),
      setVirtualDevices: async (enabled: boolean) => {
        const be = backendRef.current;
        if (!be) return;
        dispatch({ type: "settings", settings: { virtual_devices: enabled } });
        await be.setVirtualDevices(enabled).catch((e) => logError("Settings", e));
      },
    }),
    [senders, logError],
  );

  return { state, devices, selected, actions };
}
