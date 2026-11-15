import { BatteryCharging, BatteryFull, BatteryLow, BatteryMedium, BatteryWarning } from "lucide-react";
import type { BatteryInfo } from "../lib/types";

export function batteryLabel(b: BatteryInfo): string {
  return `${b.approximate ? "≈" : ""}${b.percent} %${b.charging ? " · charging" : ""}`;
}

export function BatteryBadge({ battery, size = 12 }: { battery: BatteryInfo; size?: number }) {
  const { percent, charging } = battery;
  const Icon = charging
    ? BatteryCharging
    : percent <= 10
      ? BatteryWarning
      : percent <= 30
        ? BatteryLow
        : percent <= 70
          ? BatteryMedium
          : BatteryFull;
  const tone = charging ? "text-sky-300" : percent <= 15 ? "text-rose-300" : percent <= 30 ? "text-amber-300" : "text-emerald-300";
  return (
    <span className={`inline-flex items-center gap-1 ${tone}`} title={batteryLabel(battery)}>
      <Icon size={size} />
      {percent}%
    </span>
  );
}
