import { describe, expect, it } from "vitest";
import tr from "./tr.json";
import en from "./en.json";

describe("i18n dictionaries", () => {
  it("have the same keys", () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(tr).sort());
  });
});
