import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import type { Lang } from "../lib/types";
import { en, sk, type Key } from "./strings";

export type { Key };
export type Dict = Partial<Record<Key, string>>;

/** Native and English names; `rtl` flips the layout. */
export const LANG_INFO: Record<Lang, { native: string; english: string; rtl?: boolean }> = {
  sk: { native: "Slovenčina", english: "Slovak" },
  en: { native: "English", english: "English" },
  cs: { native: "Čeština", english: "Czech" },
  hu: { native: "Magyar", english: "Hungarian" },
  "es-ES": { native: "Español (España)", english: "Spanish (Spain)" },
  "es-419": { native: "Español (Latinoamérica)", english: "Spanish (Latin America)" },
  "zh-Hans": { native: "简体中文", english: "Chinese (Simplified)" },
  "zh-Hant": { native: "繁體中文", english: "Chinese (Traditional)" },
  de: { native: "Deutsch", english: "German" },
  fr: { native: "Français", english: "French" },
  ja: { native: "日本語", english: "Japanese" },
  "pt-BR": { native: "Português (Brasil)", english: "Portuguese (Brazil)" },
  "pt-PT": { native: "Português (Portugal)", english: "Portuguese (Portugal)" },
  ko: { native: "한국어", english: "Korean" },
  ar: { native: "العربية", english: "Arabic", rtl: true },
  hi: { native: "हिन्दी", english: "Hindi" },
  it: { native: "Italiano", english: "Italian" },
  id: { native: "Bahasa Indonesia", english: "Indonesian" },
  vi: { native: "Tiếng Việt", english: "Vietnamese" },
  th: { native: "ไทย", english: "Thai" },
  tr: { native: "Türkçe", english: "Turkish" },
  pl: { native: "Polski", english: "Polish" },
  uk: { native: "Українська", english: "Ukrainian" },
  nl: { native: "Nederlands", english: "Dutch" },
  sv: { native: "Svenska", english: "Swedish" },
};

// Every language except Slovak and English is loaded on demand.
const loaders = import.meta.glob<{ default: Dict }>("./locales/*.ts");

export async function loadDict(lang: Lang): Promise<Dict> {
  if (lang === "sk") return sk;
  if (lang === "en") return en;
  const load = loaders[`./locales/${lang}.ts`];
  return load ? (await load()).default : en;
}

export type TFn = (key: Key, vars?: Record<string, string | number>) => string;

export function translateWith(dict: Dict, key: Key, vars?: Record<string, string | number>): string {
  let s = dict[key] ?? en[key] ?? key;
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}

/** Zone names reported by drivers, mapped to translatable keys. */
const ZONES: [RegExp, Key][] = [
  [/^Primary$/, "zoneName.primary"],
  [/^Primary (\d+)$/, "zoneName.primaryN"],
  [/^Logo$/, "zoneName.logo"],
  [/^Left Side$/, "zoneName.leftSide"],
  [/^Right Side$/, "zoneName.rightSide"],
  [/^Combined$/, "zoneName.combined"],
  [/^Zone (\d+)$/, "zoneName.zoneN"],
  [/^Mainboard$/, "zoneName.mainboard"],
  [/^Onboard$/, "zoneName.onboard"],
  [/^RGB Header (\d+)$/, "zoneName.rgbHeaderN"],
  [/^(?:ARGB|Addressable) Header (\d+)$/, "zoneName.argbHeaderN"],
  [/^All LEDs$/, "zoneName.allLeds"],
  [/^Strip$/, "zoneName.strip"],
  [/^Left$/, "zoneName.left"],
  [/^Right$/, "zoneName.right"],
  [/^Left Rear$/, "zoneName.leftRear"],
  [/^Right Rear$/, "zoneName.rightRear"],
];

export function translateZone(name: string, t: TFn): string {
  for (const [re, key] of ZONES) {
    const m = re.exec(name);
    if (m) return t(key, { n: m[1] ?? "" });
  }
  return name;
}

interface I18n {
  lang: Lang;
  t: TFn;
  zone: (name: string) => string;
}

const Ctx = createContext<I18n>({
  lang: "sk",
  t: (k, v) => translateWith(sk, k, v),
  zone: (n) => n,
});

export function I18nProvider({ lang, children }: { lang: Lang; children: ReactNode }) {
  const [dict, setDict] = useState<Dict>(lang === "en" ? en : sk);

  useEffect(() => {
    let live = true;
    loadDict(lang).then((d) => live && setDict(d));
    const html = document.documentElement;
    html.lang = lang;
    html.dir = LANG_INFO[lang]?.rtl ? "rtl" : "ltr";
    return () => {
      live = false;
    };
  }, [lang]);

  const t = useCallback<TFn>((k, v) => translateWith(dict, k, v), [dict]);
  const zone = useCallback((n: string) => translateZone(n, t), [t]);
  return <Ctx.Provider value={{ lang, t, zone }}>{children}</Ctx.Provider>;
}

export const useI18n = () => useContext(Ctx);
