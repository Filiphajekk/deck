import { createContext, useContext, useMemo, type ReactNode } from "react";
import cs from "./cs.json";
import en from "./en.json";

export type Lang = "cs" | "en";
type Dict = Record<string, string>;

const DICTS: Record<Lang, Dict> = { cs, en };

/** Looks up `key` (falls back to English, then to the key) and fills `{name}` placeholders. */
export function translate(lang: Lang, key: string, vars?: Record<string, string | number>): string {
  let s = DICTS[lang][key] ?? DICTS.en[key] ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  }
  return s;
}

export type TFunc = (key: string, vars?: Record<string, string | number>) => string;

const I18nContext = createContext<{ lang: Lang; t: TFunc }>({
  lang: "en",
  t: (k, v) => translate("en", k, v),
});

export function I18nProvider({ lang, children }: { lang: Lang; children: ReactNode }) {
  const value = useMemo(() => ({ lang, t: ((k, v) => translate(lang, k, v)) as TFunc }), [lang]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useT(): TFunc {
  return useContext(I18nContext).t;
}

export function useLang(): Lang {
  return useContext(I18nContext).lang;
}

/** Resolves the language preference: "auto" uses the Windows UI language. */
export function resolveLang(pref: string, systemLang: string): Lang {
  if (pref === "cs" || pref === "en") return pref;
  return systemLang === "cs" ? "cs" : "en";
}
