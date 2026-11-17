import { ALL_KEY_IDS, G910_ROWS, ROW_UNITS, isKey } from "./keyboardLayout";
import type { Rgb } from "./types";
import type { Key } from "../i18n";

export interface Template {
  id: string;
  label: Key;
  /** Suggested game executables for auto-switching. */
  games: string[];
  keys: () => Record<string, Rgb>;
}

const hex = (h: string): Rgb => {
  const n = parseInt(h.slice(1), 16);
  return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
};
const code = (c: number) => `k:${c.toString(16).padStart(2, "0")}`;
const range = (a: number, b: number) => Array.from({ length: b - a + 1 }, (_, i) => code(a + i));

/** Base colour everywhere, then highlight groups on top. */
function paint(base: Rgb, groups: [string[], Rgb][]): Record<string, Rgb> {
  const out: Record<string, Rgb> = Object.fromEntries(ALL_KEY_IDS.map((id) => [id, base]));
  for (const [ids, c] of groups) for (const id of ids) out[id] = c;
  return out;
}

const WASD = [0x1a, 0x04, 0x16, 0x07].map(code);
const ARROWS = range(0x4f, 0x52);
const DIM = hex("#0b0f1e");

export const TEMPLATES: Template[] = [
  {
    id: "fps",
    label: "tpl.fps",
    games: ["cs2.exe", "valorant-win64-shipping.exe", "r5apex.exe", "fortniteclient-win64-shipping.exe", "cod.exe"],
    keys: () =>
      paint(DIM, [
        [[...range(0x1e, 0x22), code(0x2b), code(0x29)], hex("#ff9f00")],
        [[code(0x2c), code(0xe1), code(0xe0), code(0x15), code(0x08), code(0x14), code(0x09), code(0x0a)], hex("#ffffff")],
        [WASD, hex("#ff0030")],
        [["logo", "logo2"], hex("#ff0030")],
      ]),
  },
  {
    id: "moba",
    label: "tpl.moba",
    games: ["league of legends.exe", "dota2.exe"],
    keys: () =>
      paint(DIM, [
        [[...range(0x1e, 0x23), code(0x05), code(0x2b), code(0x2c)], hex("#7c5cff")],
        [[code(0x07), code(0x09)], hex("#ffc800")],
        [[0x14, 0x1a, 0x08, 0x15].map(code), hex("#00e5ff")],
        [["logo", "logo2"], hex("#00e5ff")],
      ]),
  },
  {
    id: "mmo",
    label: "tpl.mmo",
    games: ["wow.exe", "ffxiv_dx11.exe", "gw2-64.exe"],
    keys: () =>
      paint(DIM, [
        [range(0x3a, 0x45), hex("#3b5bff")],
        [[...WASD, code(0x14), code(0x08), code(0x2b), code(0xe1)], hex("#ffffff")],
        [[...range(0x1e, 0x27), code(0x2d), code(0x2e)], hex("#00ff6a")],
        [["g:1", "g:2", "g:3", "g:4", "g:5", "g:6", "g:7", "g:8", "g:9", "logo", "logo2"], hex("#00ff6a")],
      ]),
  },
  {
    id: "racing",
    label: "tpl.racing",
    games: ["forzahorizon5.exe", "forzahorizon4.exe", "acc.exe"],
    keys: () =>
      paint(DIM, [
        [[code(0x2c), code(0xe1), code(0xe0), code(0x08), code(0x14)], hex("#ffc800")],
        [[...WASD, ...ARROWS], hex("#00ff6a")],
        [["logo", "logo2"], hex("#00ff6a")],
      ]),
  },
  {
    id: "minecraft",
    label: "tpl.minecraft",
    games: ["minecraft.windows.exe", "javaw.exe"],
    keys: () =>
      paint(hex("#08130a"), [
        [[...range(0x1e, 0x26), code(0x08), code(0x14), code(0x17), code(0x38), code(0x3c), code(0x3e)], hex("#8b5a2b")],
        [[code(0x2c), code(0xe1)], hex("#ffffff")],
        [WASD, hex("#3ccf4e")],
        [["logo", "logo2"], hex("#3ccf4e")],
      ]),
  },
  {
    id: "work",
    label: "tpl.work",
    games: [],
    keys: () => paint(hex("#c8cce0"), [[["logo", "logo2"], hex("#7c5cff")]]),
  },
  {
    id: "rainbow",
    label: "tpl.rainbow",
    games: [],
    keys: () => {
      // Hue follows the horizontal position of each key.
      const out: Record<string, Rgb> = {};
      for (const row of G910_ROWS) {
        let x = 0;
        for (const item of row) {
          const w = isKey(item) ? (item.w ?? 1) : item.gap;
          if (isKey(item)) out[item.id] = hsv((x / ROW_UNITS) * 300);
          x += w;
        }
      }
      for (let i = 1; i <= 9; i++) out[`g:${i}`] = hsv(0);
      out.logo = hsv(300);
      out.logo2 = hsv(300);
      return out;
    },
  },
];

function hsv(h: number): Rgb {
  const f = (n: number) => {
    const k = (n + h / 60) % 6;
    return Math.round((1 - Math.max(0, Math.min(k, 4 - k, 1))) * 255);
  };
  return { r: f(5), g: f(3), b: f(1) };
}
