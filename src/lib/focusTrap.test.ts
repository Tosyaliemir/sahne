// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { focusTrap } from "./focusTrap";

function setup() {
  document.body.innerHTML = `
    <button id="opener">open</button>
    <div id="dialog">
      <button id="close">x</button>
      <select id="a"><option>1</option></select>
      <button id="b">b</button>
    </div>`;
  const $ = (id: string) => document.getElementById(id) as HTMLElement;
  $("opener").focus();
  return $;
}

const tab = (shiftKey = false) =>
  document.dispatchEvent(
    new KeyboardEvent("keydown", { key: "Tab", shiftKey, bubbles: true, cancelable: true }),
  );

describe("focusTrap (A1)", () => {
  it("focuses the initial element, wraps Tab, and restores focus", () => {
    const $ = setup();
    const release = focusTrap(() => $("close"))($("dialog"));
    expect(document.activeElement).toBe($("close"));
    $("b").focus();
    tab();
    expect(document.activeElement).toBe($("close"));
    tab(true);
    expect(document.activeElement).toBe($("b"));
    release();
    expect(document.activeElement).toBe($("opener"));
  });
});
