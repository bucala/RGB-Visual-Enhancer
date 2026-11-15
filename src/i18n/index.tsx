import { createContext, useCallback, useContext, useEffect, type ReactNode } from "react";
import { en, sk, translateZone, type Key } from "./strings";

export type Lang = "sk" | "en";
export type { Key };

const DICTS: Record<Lang, Record<Key, string>> = { sk, en };

export type TFn = (key: Key, vars?: Record<string, string | number>) => string;

export function translate(lang: Lang, key: Key, vars?: Record<string, string | number>): string {
  let s = DICTS[lang][key] ?? en[key] ?? key;
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}

interface I18n {
  lang: Lang;
  t: TFn;
  zone: (name: string) => string;
}

const Ctx = createContext<I18n>({
  lang: "sk",
  t: (k, v) => translate("sk", k, v),
  zone: (n) => translateZone(n, "sk"),
});

export function I18nProvider({ lang, children }: { lang: Lang; children: ReactNode }) {
  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);
  const t = useCallback<TFn>((k, v) => translate(lang, k, v), [lang]);
  const zone = useCallback((n: string) => translateZone(n, lang), [lang]);
  return <Ctx.Provider value={{ lang, t, zone }}>{children}</Ctx.Provider>;
}

export const useI18n = () => useContext(Ctx);
