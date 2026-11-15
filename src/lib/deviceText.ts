import type { Key, TFn } from "../i18n";
import type { Device } from "./types";

export const effectKey = (id: number) => `effect.${id}` as Key;

export function connectionLabel(d: Device, t: TFn): string {
  if (d.wireless && d.transport === "usb") return t("transport.wireless");
  return t(`transport.${d.transport}` as Key);
}

export function batteryLabel(d: Device, t: TFn): string | null {
  const b = d.battery;
  if (!b) return null;
  return `${b.approximate ? "≈" : ""}${b.percent} %${b.charging ? ` · ${t("battery.charging")}` : ""}`;
}

/** Plain-text summary for "copy device info". */
export function deviceSummary(d: Device, t: TFn): string {
  const lines = [
    d.name,
    `${d.vendor} · ${connectionLabel(d, t)}`,
    `ID: ${d.id}`,
    d.location,
  ];
  if (d.firmware) lines.push(`${t("panel.firmware")}: ${d.firmware}`);
  if (d.serial) lines.push(`S/N: ${d.serial}`);
  const battery = batteryLabel(d, t);
  if (battery) lines.push(`${t("battery")}: ${battery}`);
  return lines.join("\n");
}
