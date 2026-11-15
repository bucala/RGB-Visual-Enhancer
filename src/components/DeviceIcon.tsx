import { CircleHelp, Cpu, Gpu, Headphones, Keyboard, Lightbulb, MemoryStick, Mouse, Rainbow, Speaker } from "lucide-react";
import type { DeviceType } from "../lib/types";

const ICONS = {
  motherboard: Cpu,
  gpu: Gpu,
  dram: MemoryStick,
  keyboard: Keyboard,
  mouse: Mouse,
  speaker: Speaker,
  headset: Headphones,
  led_strip: Rainbow,
  light: Lightbulb,
  unknown: CircleHelp,
} satisfies Record<DeviceType, unknown>;

export const TYPE_LABELS: Record<DeviceType, string> = {
  motherboard: "Motherboards",
  gpu: "Graphics Cards",
  dram: "Memory",
  keyboard: "Keyboards",
  mouse: "Mice",
  speaker: "Speakers",
  headset: "Headsets",
  led_strip: "LED Strips",
  light: "Lights",
  unknown: "Other",
};

export function DeviceIcon({ type, size = 18 }: { type: DeviceType; size?: number }) {
  const Icon = ICONS[type];
  return <Icon size={size} strokeWidth={1.75} />;
}
