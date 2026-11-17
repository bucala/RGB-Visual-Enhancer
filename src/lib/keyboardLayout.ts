// Visual layout of the Logitech G910 (ISO, full size) with the key ids the
// backend understands: `k:XX` = HID usage id, `g:N` = G-key, `logo`, `logo2` (palm-rest badge).

export interface KeyDef {
  id: string;
  label: string;
  /** Width in key units (default 1). */
  w?: number;
}
/** A row item: a key, or a gap of `gap` units. */
export type RowItem = KeyDef | { gap: number };

const k = (code: number, label: string, w?: number): KeyDef => ({ id: `k:${code.toString(16).padStart(2, "0")}`, label, w });
const gap = (n: number) => ({ gap: n });

const letters = (codes: [number, string][]) => codes.map(([c, l]) => k(c, l));

/** Main block + navigation + numpad, row by row. */
export const G910_ROWS: RowItem[][] = [
  [
    k(0x29, "Esc"), gap(1),
    k(0x3a, "F1"), k(0x3b, "F2"), k(0x3c, "F3"), k(0x3d, "F4"), gap(0.5),
    k(0x3e, "F5"), k(0x3f, "F6"), k(0x40, "F7"), k(0x41, "F8"), gap(0.5),
    k(0x42, "F9"), k(0x43, "F10"), k(0x44, "F11"), k(0x45, "F12"), gap(0.25),
    k(0x46, "PrtSc"), k(0x47, "ScrLk"), k(0x48, "Pause"),
  ],
  [
    k(0x35, "`"), ...letters([[0x1e, "1"], [0x1f, "2"], [0x20, "3"], [0x21, "4"], [0x22, "5"], [0x23, "6"], [0x24, "7"], [0x25, "8"], [0x26, "9"], [0x27, "0"]]),
    k(0x2d, "-"), k(0x2e, "="), k(0x2a, "⌫", 2), gap(0.25),
    k(0x49, "Ins"), k(0x4a, "Home"), k(0x4b, "PgUp"), gap(0.25),
    k(0x53, "Num"), k(0x54, "/"), k(0x55, "*"), k(0x56, "-"),
  ],
  [
    k(0x2b, "Tab", 1.5), ...letters([[0x14, "Q"], [0x1a, "W"], [0x08, "E"], [0x15, "R"], [0x17, "T"], [0x1c, "Y"], [0x18, "U"], [0x0c, "I"], [0x12, "O"], [0x13, "P"]]),
    k(0x2f, "["), k(0x30, "]"), k(0x28, "Enter", 1.5), gap(0.25),
    k(0x4c, "Del"), k(0x4d, "End"), k(0x4e, "PgDn"), gap(0.25),
    k(0x5f, "7"), k(0x60, "8"), k(0x61, "9"), k(0x57, "+"),
  ],
  [
    k(0x39, "Caps", 1.75), ...letters([[0x04, "A"], [0x16, "S"], [0x07, "D"], [0x09, "F"], [0x0a, "G"], [0x0b, "H"], [0x0d, "J"], [0x0e, "K"], [0x0f, "L"]]),
    k(0x33, ";"), k(0x34, "'"), k(0x32, "#"), gap(1.25), gap(0.25),
    gap(3), gap(0.25),
    k(0x5c, "4"), k(0x5d, "5"), k(0x5e, "6"), gap(1),
  ],
  [
    k(0xe1, "Shift", 1.25), k(0x64, "\\"), ...letters([[0x1d, "Z"], [0x1b, "X"], [0x06, "C"], [0x19, "V"], [0x05, "B"], [0x11, "N"], [0x10, "M"]]),
    k(0x36, ","), k(0x37, "."), k(0x38, "/"), k(0xe5, "Shift", 2.75), gap(0.25),
    gap(1), k(0x52, "↑"), gap(1), gap(0.25),
    k(0x59, "1"), k(0x5a, "2"), k(0x5b, "3"), k(0x58, "Ent"),
  ],
  [
    k(0xe0, "Ctrl", 1.25), k(0xe3, "Win", 1.25), k(0xe2, "Alt", 1.25), k(0x2c, "", 6.25),
    k(0xe6, "AltGr", 1.25), k(0xe7, "Win", 1.25), k(0x65, "Menu", 1.25), k(0xe4, "Ctrl", 1.25), gap(0.25),
    k(0x50, "←"), k(0x51, "↓"), k(0x4f, "→"), gap(0.25),
    k(0x62, "0", 2), k(0x63, "."), gap(1),
  ],
];

/** G-keys (column on the left on the G910). */
export const G910_GKEYS: KeyDef[] = Array.from({ length: 9 }, (_, i) => ({ id: `g:${i + 1}`, label: `G${i + 1}` }));
export const G910_LOGO: KeyDef = { id: "logo", label: "G" };
/** Second logo LED (code 0x02), most likely the lit "G910" palm-rest badge. */
export const G910_BADGE: KeyDef = { id: "logo2", label: "G910" };

export function isKey(item: RowItem): item is KeyDef {
  return "id" in item;
}

export const ALL_KEYS: KeyDef[] = [...G910_ROWS.flat().filter(isKey), ...G910_GKEYS, G910_LOGO, G910_BADGE];
export const ALL_KEY_IDS = ALL_KEYS.map((k) => k.id);

/** Row width in units (all rows are padded to the same width). */
export const ROW_UNITS = 22.5;
