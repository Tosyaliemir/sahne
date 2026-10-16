import tr from "./tr.json";
import en from "./en.json";

export type Lang = "tr" | "en";
/** The stored preference: a language, or "system" to follow the OS locale. */
export type LanguageSetting = "system" | Lang;
/** `en.json` is the source of truth; every other dictionary has the same keys. */
export type MessageKey = keyof typeof en;

const dictionaries: Record<Lang, Record<MessageKey, string>> = { tr, en };

const state = $state({ lang: "en" as Lang });

/**
 * The UI language for a setting. "system" picks Turkish when the preferred
 * system locale is Turkish and English otherwise.
 */
export function resolveLang(
  setting: LanguageSetting,
  locales: readonly string[] = typeof navigator === "undefined" ? [] : navigator.languages,
): Lang {
  if (setting !== "system") return setting;
  return locales[0]?.toLowerCase().startsWith("tr") ? "tr" : "en";
}

export function setLang(lang: Lang): void {
  state.lang = lang;
}

export function currentLang(): Lang {
  return state.lang;
}

/** Looks up a UI string; `{name}` placeholders are filled from `params`. */
export function t(key: MessageKey, params: Record<string, string> = {}): string {
  const text = dictionaries[state.lang][key] ?? dictionaries.en[key] ?? key;
  return text.replace(/\{(\w+)\}/g, (_, name: string) => params[name] ?? `{${name}}`);
}
