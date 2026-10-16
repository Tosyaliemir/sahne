import { describe, expect, it, vi } from "vitest";
import { Lifecycle } from "./lifecycle";

describe("Lifecycle", () => {
  it("runs cleanups that resolve after dispose (C10)", async () => {
    const life = new Lifecycle();
    let resolve!: (stop: () => void) => void;
    const late = new Promise<() => void>((r) => (resolve = r));
    const early = vi.fn();
    const lateStop = vi.fn();
    life.add(early);
    life.add(late);
    life.dispose();
    expect(early).toHaveBeenCalledOnce();
    expect(lateStop).not.toHaveBeenCalled();
    resolve(lateStop);
    await late;
    await Promise.resolve();
    expect(lateStop).toHaveBeenCalledOnce();
  });

  it("calls each cleanup once and survives a throwing one", async () => {
    const life = new Lifecycle();
    const a = vi.fn();
    const b = vi.fn(() => {
      throw new Error("boom");
    });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    life.add(Promise.resolve(a));
    life.add(b);
    await Promise.resolve();
    life.dispose();
    life.dispose();
    expect(a).toHaveBeenCalledOnce();
    expect(b).toHaveBeenCalledOnce();
    warn.mockRestore();
  });

  it("unlistens every store listener even when unmounted mid-setup", async () => {
    const unlisteners: ReturnType<typeof vi.fn>[] = [];
    vi.resetModules();
    vi.doMock("@tauri-apps/api/event", () => ({
      listen: vi.fn(async () => {
        await new Promise((r) => setTimeout(r, 5));
        const u = vi.fn();
        unlisteners.push(u);
        return u;
      }),
    }));
    vi.doMock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
    const { player } = await import("./stores/player.svelte");
    const life = new Lifecycle();
    const started = player.start();
    life.add(started);
    life.dispose(); // unmounted before the listeners exist
    await started;
    await new Promise((r) => setTimeout(r, 0));
    expect(unlisteners.length).toBeGreaterThan(0);
    for (const u of unlisteners) expect(u).toHaveBeenCalledOnce();
    vi.doUnmock("@tauri-apps/api/event");
    vi.doUnmock("@tauri-apps/api/core");
  });
});
