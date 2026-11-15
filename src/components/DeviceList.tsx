import { Bluetooth, Cable, CircuitBoard, FlaskConical } from "lucide-react";
import { rgbCss } from "../lib/color";
import type { Device, DeviceType, Transport } from "../lib/types";
import { DeviceIcon, TYPE_LABELS } from "./DeviceIcon";

const ORDER: DeviceType[] = ["motherboard", "dram", "keyboard", "mouse", "headset", "speaker", "led_strip", "light", "unknown"];

const TRANSPORT_ICON: Record<Transport, typeof Cable> = {
  usb: Cable,
  smbus: CircuitBoard,
  ble: Bluetooth,
  virtual: FlaskConical,
};

export function DeviceList({ devices, selectedId, onSelect, scanning }: {
  devices: Device[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  scanning: boolean;
}) {
  const groups = ORDER.map((t) => [t, devices.filter((d) => d.device_type === t)] as const).filter(([, l]) => l.length);

  return (
    <nav className="flex min-h-0 flex-col gap-4 overflow-y-auto p-3" aria-label="Devices">
      {groups.length === 0 && (
        <div className="rounded-xl border border-dashed border-line p-5 text-center text-xs text-muted">
          {scanning ? "Looking for hardware…" : "No devices found. Check the activity log, or enable virtual devices."}
        </div>
      )}
      {groups.map(([type, list]) => (
        <div key={type}>
          <h3 className="mb-1.5 px-2 text-[10px] font-semibold tracking-[0.14em] text-muted uppercase">
            {TYPE_LABELS[type]}
          </h3>
          <ul className="flex flex-col gap-1">
            {list.map((d) => {
              const TransportIcon = TRANSPORT_ICON[d.transport];
              const active = d.id === selectedId;
              const error = d.status.state === "error";
              return (
                <li key={d.id}>
                  <button
                    type="button"
                    onClick={() => onSelect(d.id)}
                    className={`group flex w-full items-center gap-3 rounded-xl px-2.5 py-2 text-left transition ${
                      active ? "bg-surface-2 ring-1 ring-accent/50" : "hover:bg-surface-2/70"
                    }`}
                  >
                    <span className="relative flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-bg text-text">
                      <DeviceIcon type={d.device_type} />
                      <span
                        className="absolute -right-0.5 -bottom-0.5 h-3 w-3 rounded-full border-2 border-surface"
                        style={{ background: d.color ? rgbCss(d.color) : "#3a4150" }}
                      />
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[13px] font-medium">{d.name}</span>
                      <span className="flex items-center gap-1 text-[11px] text-muted">
                        <TransportIcon size={11} />
                        {d.vendor}
                      </span>
                    </span>
                    <span
                      title={d.status.state === "error" ? d.status.message : "Ready"}
                      className={`h-2 w-2 shrink-0 rounded-full ${error ? "bg-rose-500" : "bg-emerald-400"}`}
                    />
                  </button>
                </li>
              );
            })}
          </ul>
        </div>
      ))}
    </nav>
  );
}
