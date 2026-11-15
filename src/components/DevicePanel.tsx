import { Layers, Palette, Sparkles } from "lucide-react";
import { useEffect, useState } from "react";
import type { Device, Rgb } from "../lib/types";
import { ColorPicker } from "./ColorPicker";
import { DeviceIcon } from "./DeviceIcon";
import { EffectPanel } from "./EffectPanel";
import { Card, Chip } from "./ui";
import { ZoneList } from "./ZoneList";

const TRANSPORT_LABEL = { usb: "USB HID", smbus: "SMBus", i2c: "GPU I²C", ble: "Bluetooth LE", virtual: "Simulated" };
const DEFAULT_COLOR: Rgb = { r: 124, g: 92, b: 255 };

export function DevicePanel({ device, syncAll, syncCount, onColor, onZoneColor, onEffect }: {
  device: Device;
  syncAll: boolean;
  syncCount: number;
  onColor: (c: Rgb) => void;
  onZoneColor: (zone: number, c: Rgb) => void;
  onEffect: (effect: number, speed: number) => void;
}) {
  // The picker owns the "draft" colour; hardware state follows asynchronously.
  const [color, setColor] = useState<Rgb>(device.color ?? DEFAULT_COLOR);
  // Reset only when switching devices, not on every hardware echo.
  useEffect(() => setColor(device.color ?? DEFAULT_COLOR), [device.id]);

  const activeEffect = device.effects.find((e) => e.id === device.effect);
  const colorRelevant = !activeEffect || activeEffect.uses_color;
  const canColor = device.effects.some((e) => e.uses_color);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-start gap-4 rounded-2xl border border-line bg-surface p-4">
        <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-bg text-text">
          <DeviceIcon type={device.device_type} size={24} />
        </div>
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-lg font-semibold">{device.name}</h2>
          <div className="mt-1.5 flex flex-wrap gap-1.5">
            <Chip>{device.vendor}</Chip>
            <Chip>{TRANSPORT_LABEL[device.transport]}</Chip>
            {device.firmware && <Chip>fw {device.firmware}</Chip>}
            {device.status.state === "error" ? (
              <Chip tone="error">Error</Chip>
            ) : (
              <Chip tone="ok">Ready</Chip>
            )}
          </div>
          <p className="mt-2 truncate font-mono text-[11px] text-muted" title={device.location}>
            {device.location}
          </p>
        </div>
      </div>

      {device.status.state === "error" && (
        <div className="rounded-xl border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-xs text-rose-200">
          {device.status.message}
        </div>
      )}

      <div className="grid gap-4 xl:grid-cols-2">
        <Card
          title="Colour"
          icon={<Palette size={15} />}
          actions={syncAll ? <Chip tone="warn">Syncing {syncCount} devices</Chip> : undefined}
        >
          {canColor ? (
            <ColorPicker
              value={color}
              onChange={(c) => {
                setColor(c);
                onColor(c);
              }}
            />
          ) : (
            <p className="text-xs text-muted">This device has no colour-based effects.</p>
          )}
          {canColor && !colorRelevant && (
            <p className="mt-3 text-[11px] text-muted">
              {activeEffect?.name} ignores colour; picking one switches to Static.
            </p>
          )}
        </Card>

        <Card title="Effects" icon={<Sparkles size={15} />}>
          <EffectPanel device={device} onEffect={onEffect} />
        </Card>
      </div>

      <Card title={`Zones (${device.zones.length})`} icon={<Layers size={15} />}>
        <ZoneList device={device} color={color} onApply={(z) => onZoneColor(z, color)} />
      </Card>
    </div>
  );
}
