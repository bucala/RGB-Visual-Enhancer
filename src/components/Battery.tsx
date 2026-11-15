import { BatteryCharging, BatteryFull, BatteryLow, BatteryMedium, BatteryWarning } from "lucide-react";
import type { BatteryInfo } from "../lib/types";

export function batteryTone(b: BatteryInfo): string {
  return b.charging ? "text-sky-300" : b.percent <= 15 ? "text-rose-300" : b.percent <= 30 ? "text-amber-300" : "text-emerald-300";
}

export function BatteryBadge({ battery, size = 12 }: { battery: BatteryInfo; size?: number }) {
  const { percent, charging, approximate } = battery;
  const Icon = charging
    ? BatteryCharging
    : percent <= 10
      ? BatteryWarning
      : percent <= 30
        ? BatteryLow
        : percent <= 70
          ? BatteryMedium
          : BatteryFull;
  return (
    <span className={`inline-flex items-center gap-1 ${batteryTone(battery)}`}>
      <Icon size={size} />
      {approximate ? "≈" : ""}
      {percent}%
    </span>
  );
}
