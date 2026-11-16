// Verifies every locale covers all UI strings with matching {placeholders}.
// Usage: node scripts/check-locales.mjs
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const strings = fs.readFileSync(path.join(root, "src/i18n/strings.ts"), "utf8");
const parse = (src) =>
  Object.fromEntries([...src.matchAll(/^\s*"([^"]+)":\s*("(?:[^"\\]|\\.)*"),?\s*$/gm)].map((m) => [m[1], JSON.parse(m[2])]));
const en = parse(strings.slice(strings.indexOf("export const en = {"), strings.indexOf("} as const;")));
// Technical terms identical in every language may fall back to English.
const SAME = new Set(["transport.usb", "transport.smbus", "transport.ble", "panel.firmware", "hw.bios", "tpl.minecraft", "tpl.fps", "tpl.moba"]);
const vars = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(",");

let failed = false;
const dir = path.join(root, "src/i18n/locales");
for (const file of fs.readdirSync(dir).sort()) {
  const dict = parse(fs.readFileSync(path.join(dir, file), "utf8"));
  const problems = [];
  for (const k of Object.keys(dict)) {
    if (!(k in en)) problems.push(`unknown key ${k}`);
    else if (vars(dict[k]) !== vars(en[k])) problems.push(`placeholders differ in ${k}`);
  }
  for (const k of Object.keys(en)) if (!(k in dict) && !SAME.has(k)) problems.push(`missing ${k}`);
  console.log(`${file.padEnd(12)} ${Object.keys(dict).length} strings ${problems.length ? "✗" : "✓"}`);
  for (const p of problems) console.log(`   ${p}`);
  failed ||= problems.length > 0;
}
process.exit(failed ? 1 : 0);
