import { Bluetooth, Cable, CircuitBoard, ClipboardCopy, FlaskConical, Power, Sparkles, Wifi } from "lucide-react";
import { useI18n, type Key } from "../i18n";
import { PRESETS, rgbCss } from "../lib/color";
import { deviceSummary, effectKey } from "../lib/deviceText";
import type { Device, DeviceType, Rgb, Transport } from "../lib/types";
import { BatteryBadge } from "./Battery";
import { copyText, useContextMenu, type CtxItem } from "./ContextMenu";
import { DeviceIcon } from "./DeviceIcon";

const ORDER: DeviceType[] = ["motherboard", "gpu", "dram", "keyboard", "mouse", "headset", "speaker", "led_strip", "light", "unknown"];

const TRANSPORT_ICON: Record<Transport, typeof Cable> = {
  usb: Cable,
  smbus: CircuitBoard,
  i2c: CircuitBoard,
  ble: Bluetooth,
  virtual: FlaskConical,
};

export function DeviceList({ devices, selectedId, onSelect, scanning, onColor, onEffect }: {
  devices: Device[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  scanning: boolean;
  onColor: (id: string, c: Rgb) => void;
  onEffect: (id: string, effect: number, speed: number) => void;
}) {
  const { t } = useI18n();
  const ctx = useContextMenu();
  const groups = ORDER.map((type) => [type, devices.filter((d) => d.device_type === type)] as const).filter(([, l]) => l.length);

  const menuFor = (d: Device): CtxItem[] => {
    const items: CtxItem[] = [{ kind: "header", label: d.name }];
    if (d.effects.some((e) => e.uses_color)) {
      items.push({ kind: "swatches", colors: PRESETS, onPick: (c) => onColor(d.id, c) });
    }
    const animated = d.effects.filter((e) => e.id !== 0);
    if (animated.length) {
      items.push({ kind: "separator" });
      for (const e of animated) {
        items.push({
          kind: "item",
          label: t(effectKey(e.id)),
          icon: <Sparkles size={13} />,
          checked: d.effect === e.id,
          onSelect: () => onEffect(d.id, e.id, d.speed),
        });
      }
    }
    if (d.effects.some((e) => e.id === 0)) {
      items.push({ kind: "item", label: t("ctx.lightsOff"), icon: <Power size={13} />, onSelect: () => onEffect(d.id, 0, d.speed) });
    }
    items.push({ kind: "separator" });
    items.push({
      kind: "item",
      label: t("ctx.copyInfo"),
      icon: <ClipboardCopy size={13} />,
      onSelect: async () => {
        if (await copyText(deviceSummary(d, t))) ctx.notify(t("ctx.copied"));
      },
    });
    return items;
  };

  return (
    <nav className="flex min-h-0 flex-col gap-4 overflow-y-auto p-3" aria-label={t("list.label")}>
      {groups.length === 0 && (
        <div className="rounded-xl border border-dashed border-line p-5 text-center text-xs text-muted">
          {scanning ? t("list.scanning") : t("list.empty")}
        </div>
      )}
      {groups.map(([type, list]) => (
        <div key={type}>
          <h3 className="mb-1.5 px-2 text-[10px] font-semibold tracking-[0.14em] text-muted uppercase">
            {t(`type.${type}` as Key)}
          </h3>
          <ul className="flex flex-col gap-1">
            {list.map((d) => {
              const TransportIcon = d.wireless && d.transport === "usb" ? Wifi : TRANSPORT_ICON[d.transport];
              const active = d.id === selectedId;
              const error = d.status.state === "error";
              const lit = d.color && d.effect !== 0;
              return (
                <li key={d.id}>
                  <button
                    type="button"
                    onClick={() => onSelect(d.id)}
                    onContextMenu={(e) => {
                      onSelect(d.id);
                      ctx.open(e, menuFor(d));
                    }}
                    className={`group flex w-full items-center gap-3 rounded-xl px-2.5 py-2 text-left transition ${
                      active ? "bg-surface-2 ring-1 ring-accent/50" : "hover:bg-surface-2/70"
                    }`}
                  >
                    <span className="relative flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-bg text-text">
                      <DeviceIcon type={d.device_type} />
                      <span
                        className="absolute -right-0.5 -bottom-0.5 h-3 w-3 rounded-full border-2 border-surface"
                        style={{ background: lit && d.color ? rgbCss(d.color) : "#3a4150" }}
                      />
                    </span>
                    <span className="min-w-0 flex-1">
                      <span dir="auto" className="block truncate rtl:text-right text-[13px] font-medium">{d.name}</span>
                      <span className="flex items-center gap-1 text-[11px] text-muted">
                        <TransportIcon size={11} />
                        {d.vendor}
                        {d.battery && (
                          <span className="ml-auto pl-2">
                            <BatteryBadge battery={d.battery} size={11} />
                          </span>
                        )}
                      </span>
                    </span>
                    <span
                      title={d.status.state === "error" ? d.status.message : t("status.ready")}
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
