import tr from "./tr.json";
import en from "./en.json";

export type Lang = "tr" | "en";
export type MessageKey = keyof typeof tr;

const dictionaries: Record<Lang, Record<MessageKey, string>> = { tr, en };

const state = $state({ lang: "tr" as Lang });

export function setLang(lang: Lang): void {
  state.lang = lang;
}

/** Looks up a UI string; `{name}` placeholders are filled from `params`. */
export function t(key: MessageKey, params: Record<string, string> = {}): string {
  const text = dictionaries[state.lang][key] ?? dictionaries.tr[key] ?? key;
  return text.replace(/\{(\w+)\}/g, (_, name: string) => params[name] ?? `{${name}}`);
}
