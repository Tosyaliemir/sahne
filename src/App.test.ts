// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { fireEvent } from "@testing-library/svelte";
import { commands, lyricsIn, mockBackend, playing, renderApp, settle } from "./test/backend";

let cleanup: (() => void) | undefined;
afterEach(() => cleanup?.());

async function show(state: Parameters<typeof mockBackend>[0] = {}) {
  const calls = mockBackend(state);
  const view = await renderApp();
  cleanup = view.cleanup;
  await settle();
  return { ...view, calls };
}

const key = (k: string) => fireEvent.keyDown(window, { key: k });

describe("status screens (T1)", () => {
  it("no player", async () => {
    const { getByText } = await show({ snapshot: null });
    expect(getByText("Spotify isn't open")).toBeTruthy();
  });

  it("player without a track", async () => {
    const { container } = await show({ snapshot: { ...playing(), track: null } });
    expect(container.textContent).toContain("Nothing is playing");
  });

  it.each([
    ["loading", "Looking for lyrics"],
    ["offline", "Offline"],
    ["instrumental", "Instrumental"],
    ["not_found", "No lyrics found"],
  ] as const)("%s", async (state, text) => {
    const { container } = await show({ lyrics: lyricsIn(state) });
    expect(container.textContent).toContain(text);
    expect(container.textContent).toContain("Işıklı Günler");
  });

  it("error offers a retry", async () => {
    const { getByRole, calls } = await show({ lyrics: lyricsIn("error") });
    await fireEvent.click(getByRole("button", { name: "Try again" }));
    expect(commands(calls, "retry_lyrics")).toHaveLength(1);
  });

  it("synced lyrics show the lines", async () => {
    const { container } = await show();
    // Each word is drawn twice (base and fill layer); check the words.
    for (const word of ["Gölgede", "üşüyen", "İstanbul"]) expect(container.textContent).toContain(word);
  });
});

describe("keyboard shortcuts (T1)", () => {
  it("drive playback", async () => {
    const { calls } = await show();
    await key(" ");
    await key("ArrowRight");
    await key("ArrowLeft");
    await settle();
    expect(calls.map((c) => c.cmd)).toEqual(expect.arrayContaining(["play_pause", "next", "previous"]));
  });

  it("change the volume", async () => {
    const { calls } = await show();
    await key("ArrowUp");
    await settle();
    const [set] = commands(calls, "set_volume");
    expect(set.args?.volume).toBeCloseTo(0.55);
  });

  it("nudge this song's offset (T1: offset nudge)", async () => {
    const { calls } = await show();
    await key("]");
    await key("]");
    await key("[");
    await settle();
    const offsets = commands(calls, "set_lyrics_offset").map((c) => c.args?.offsetMs);
    expect(offsets).toEqual([100, 200, 100]);
  });

  it("cycle the style and save it", async () => {
    const { calls } = await show();
    await key("s");
    await settle(700); // settings are saved after a short debounce
    const patches = commands(calls, "update_settings").map((c) => c.args?.patch);
    expect(patches).toContainEqual({ lyrics_style: "kinetic" });
  });

  it("open and close the settings panel", async () => {
    const { queryByRole } = await show();
    await key(",");
    await settle();
    expect(queryByRole("complementary", { name: "Settings" })).toBeTruthy();
    await key("Escape");
    await settle(400);
    expect(queryByRole("complementary", { name: "Settings" })).toBeNull();
  });

  it("toggle the mini player", async () => {
    const { calls } = await show();
    await key("m");
    await settle();
    expect(commands(calls, "set_mini_mode")[0]?.args).toEqual({ enabled: true });
  });
});

describe("settings panel (T1)", () => {
  it("saves a changed backdrop and clears the cache", async () => {
    const { container, getByRole, calls } = await show();
    await key(",");
    await settle();
    const backdrop = getByRole("combobox", { name: "Backdrop" }) as HTMLSelectElement;
    await fireEvent.change(backdrop, { target: { value: "color" } });
    await fireEvent.click(getByRole("button", { name: "Clear" }));
    await settle(700);
    expect(commands(calls, "update_settings").map((c) => c.args?.patch)).toContainEqual({
      backdrop_style: "color",
    });
    expect(commands(calls, "clear_cache")).toHaveLength(1);
    expect(container.textContent).toContain("About");
  });
});

describe("frame scheduler in the app (T1, P1)", () => {
  it("keeps drawing while playing and stops when paused", async () => {
    await show({ snapshot: playing("playing") });
    const { frames } = await import("./lib/frames");
    await settle(100);
    expect(frames.running).toBe(true);
    cleanup?.();

    await show({ snapshot: playing("paused") });
    const fresh = await import("./lib/frames");
    await settle(300);
    expect(fresh.frames.running).toBe(false);
  });
});
