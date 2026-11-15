import { Paintbrush } from "lucide-react";
import { useI18n } from "../i18n";
import { PRESETS, rgbCss } from "../lib/color";
import type { Device, Rgb } from "../lib/types";
import { useContextMenu } from "./ContextMenu";

export function ZoneList({ device, color, onApply }: { device: Device; color: Rgb; onApply: (zone: number, c: Rgb) => void }) {
  const { t, zone } = useI18n();
  const ctx = useContextMenu();
  if (device.zones.length === 0) return <p className="text-xs text-muted">{t("zone.none")}</p>;
  return (
    <ul className="grid gap-2 sm:grid-cols-2">
      {device.zones.map((z) => (
        <li
          key={z.index}
          className="flex items-center gap-3 rounded-xl border border-line bg-bg px-3 py-2"
          onContextMenu={(e) =>
            ctx.open(e, [
              { kind: "header", label: zone(z.name) },
              { kind: "item", label: t("ctx.paintZone"), icon: <Paintbrush size={13} />, onSelect: () => onApply(z.index, color) },
              { kind: "swatches", colors: PRESETS, onPick: (c) => onApply(z.index, c) },
            ])
          }
        >
          <span className="font-mono text-[11px] text-muted">#{z.index + 1}</span>
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[13px]">{zone(z.name)}</span>
            <span className="text-[11px] text-muted">
              {z.led_count > 0 ? t("zone.leds", { n: z.led_count }) : t("zone.ledsUnknown")}
            </span>
          </span>
          <button
            type="button"
            onClick={() => onApply(z.index, color)}
            title={t("zone.paint")}
            aria-label={t("zone.paint")}
            className="flex items-center gap-1.5 rounded-lg border border-line px-2 py-1 text-[11px] text-muted transition hover:border-accent/60 hover:text-text"
          >
            <span className="h-3 w-3 rounded-full" style={{ background: rgbCss(color) }} />
            <Paintbrush size={12} />
          </button>
        </li>
      ))}
    </ul>
  );
}
