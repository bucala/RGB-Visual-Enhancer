import type { Rgb } from "./types";

export interface Hsv {
  h: number; // 0–360
  s: number; // 0–1
  v: number; // 0–1
}

export function hsvToRgb({ h, s, v }: Hsv): Rgb {
  const f = (n: number) => {
    const k = (n + h / 60) % 6;
    return v - v * s * Math.max(0, Math.min(k, 4 - k, 1));
  };
  return { r: Math.round(f(5) * 255), g: Math.round(f(3) * 255), b: Math.round(f(1) * 255) };
}

export function rgbToHsv({ r, g, b }: Rgb): Hsv {
  const rn = r / 255, gn = g / 255, bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const d = max - Math.min(rn, gn, bn);
  let h = 0;
  if (d !== 0) {
    if (max === rn) h = ((gn - bn) / d) % 6;
    else if (max === gn) h = (bn - rn) / d + 2;
    else h = (rn - gn) / d + 4;
    h *= 60;
    if (h < 0) h += 360;
  }
  return { h, s: max === 0 ? 0 : d / max, v: max };
}

export function rgbToHex({ r, g, b }: Rgb): string {
  return "#" + [r, g, b].map((n) => n.toString(16).padStart(2, "0")).join("");
}

export function hexToRgb(hex: string): Rgb | null {
  const m = /^#?([0-9a-f]{6}|[0-9a-f]{3})$/i.exec(hex.trim());
  if (!m) return null;
  let h = m[1]!;
  if (h.length === 3) h = [...h].map((c) => c + c).join("");
  const n = parseInt(h, 16);
  return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
}

export const rgbCss = (c: Rgb) => `rgb(${c.r} ${c.g} ${c.b})`;

export const PRESETS: Rgb[] = [
  { r: 255, g: 0, b: 0 },
  { r: 255, g: 96, b: 0 },
  { r: 255, g: 200, b: 0 },
  { r: 0, g: 255, b: 64 },
  { r: 0, g: 220, b: 255 },
  { r: 0, g: 64, b: 255 },
  { r: 140, g: 0, b: 255 },
  { r: 255, g: 0, b: 160 },
  { r: 255, g: 255, b: 255 },
];
