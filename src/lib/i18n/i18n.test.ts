import { describe, expect, it } from "vitest";
import tr from "./tr.json";
import en from "./en.json";
import { resolveLang } from "./index.svelte";

const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

describe("i18n dictionaries", () => {
  it("have the same keys", () => {
    expect(Object.keys(tr).sort()).toEqual(Object.keys(en).sort());
  });

  it("use the same placeholders for every key", () => {
    for (const [key, text] of Object.entries(en)) {
      expect(placeholders(tr[key as keyof typeof tr]), key).toEqual(placeholders(text));
    }
  });
});

describe("resolveLang", () => {
  it("keeps an explicit choice", () => {
    expect(resolveLang("tr", ["en-US"])).toBe("tr");
    expect(resolveLang("en", ["tr-TR"])).toBe("en");
  });

  it("follows the preferred system locale, falling back to English", () => {
    expect(resolveLang("system", ["tr-TR", "en-US"])).toBe("tr");
    expect(resolveLang("system", ["TR"])).toBe("tr");
    expect(resolveLang("system", ["de-DE", "tr-TR"])).toBe("en");
    expect(resolveLang("system", [])).toBe("en");
  });
});
