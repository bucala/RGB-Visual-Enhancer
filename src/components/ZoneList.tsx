import { Paintbrush } from "lucide-react";
import { rgbCss } from "../lib/color";
import type { Device, Rgb } from "../lib/types";

export function ZoneList({ device, color, onApply }: { device: Device; color: Rgb; onApply: (zone: number) => void }) {
  if (device.zones.length === 0) return <p className="text-xs text-muted">This device exposes no zones.</p>;
  return (
    <ul className="grid gap-2 sm:grid-cols-2">
      {device.zones.map((z) => (
        <li key={z.index} className="flex items-center gap-3 rounded-xl border border-line bg-bg px-3 py-2">
          <span className="font-mono text-[11px] text-muted">#{z.index + 1}</span>
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[13px]">{z.name}</span>
            <span className="text-[11px] text-muted">{z.led_count > 0 ? `${z.led_count} LED${z.led_count === 1 ? "" : "s"}` : "LED count unknown"}</span>
          </span>
          <button
            type="button"
            onClick={() => onApply(z.index)}
            title="Paint this zone with the selected colour"
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
