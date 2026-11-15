import { Layers, Palette, Sparkles } from "lucide-react";
import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import { batteryLabel, connectionLabel, effectKey } from "../lib/deviceText";
import type { Device, Rgb } from "../lib/types";
import { ColorPicker } from "./ColorPicker";
import { DeviceIcon } from "./DeviceIcon";
import { EffectPanel } from "./EffectPanel";
import { Card, Chip } from "./ui";
import { ZoneList } from "./ZoneList";

const DEFAULT_COLOR: Rgb = { r: 124, g: 92, b: 255 };

export function DevicePanel({ device, syncAll, syncCount, onColor, onApplyAll, onZoneColor, onEffect }: {
  device: Device;
  syncAll: boolean;
  syncCount: number;
  onColor: (c: Rgb) => void;
  onApplyAll: (c: Rgb) => void;
  onZoneColor: (zone: number, c: Rgb) => void;
  onEffect: (effect: number, speed: number) => void;
}) {
  const { t } = useI18n();
  // The picker owns the "draft" colour; hardware state follows asynchronously.
  const [color, setColor] = useState<Rgb>(device.color ?? DEFAULT_COLOR);
  // Reset only when switching devices, not on every hardware echo.
  useEffect(() => setColor(device.color ?? DEFAULT_COLOR), [device.id]);

  const activeEffect = device.effects.find((e) => e.id === device.effect);
  const colorRelevant = !activeEffect || activeEffect.uses_color;
  const canColor = device.effects.some((e) => e.uses_color);
  const battery = batteryLabel(device, t);
  const b = device.battery;

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
            <Chip>{connectionLabel(device, t)}</Chip>
            {b && battery && (
              <Chip tone={b.percent <= 15 && !b.charging ? "error" : b.percent <= 30 ? "warn" : "ok"}>
                {t("battery")} {battery}
              </Chip>
            )}
            {device.firmware && (
              <Chip>
                {t("panel.firmware")} {device.firmware}
              </Chip>
            )}
            {device.status.state === "error" ? (
              <Chip tone="error">{t("status.error")}</Chip>
            ) : (
              <Chip tone="ok">{t("status.ready")}</Chip>
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
          title={t("panel.colour")}
          icon={<Palette size={15} />}
          actions={syncAll ? <Chip tone="warn">{t("panel.syncing", { n: syncCount })}</Chip> : undefined}
        >
          {canColor ? (
            <ColorPicker
              value={color}
              onChange={(c) => {
                setColor(c);
                onColor(c);
              }}
              onApplyAll={onApplyAll}
            />
          ) : (
            <p className="text-xs text-muted">{t("panel.noColour")}</p>
          )}
          {canColor && !colorRelevant && activeEffect && (
            <p className="mt-3 text-[11px] text-muted">{t("panel.ignoresColour", { effect: t(effectKey(activeEffect.id)) })}</p>
          )}
        </Card>

        <Card title={t("panel.effects")} icon={<Sparkles size={15} />}>
          <EffectPanel device={device} onEffect={onEffect} />
        </Card>
      </div>

      <Card title={t("panel.zones", { n: device.zones.length })} icon={<Layers size={15} />}>
        <ZoneList device={device} color={color} onApply={onZoneColor} />
      </Card>
    </div>
  );
}
