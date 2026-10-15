import { describe, expect, it } from "vitest";
import { HARD_THRESHOLD_MS, PlaybackClock, SMOOTH_MS } from "./clock";

const sync = (position_ms: number, playing = true, hard = false) => ({ position_ms, playing, rate: 1, hard });

describe("PlaybackClock", () => {
  it("extrapolates while playing and freezes while paused", () => {
    const c = new PlaybackClock();
    c.sync(sync(10_000, true, true), 0);
    expect(c.position(250)).toBe(10_250);
    c.sync(sync(10_500, false), 500);
    expect(c.position(5_000)).toBe(10_500);
  });

  it("smooths small drift without jumping", () => {
    const c = new PlaybackClock();
    c.sync(sync(0, true, true), 0);
    // Backend says we are 200 ms ahead of our estimate.
    c.sync(sync(1_200), 1_000);
    expect(c.position(1_000)).toBe(1_000); // continuous at the sync point
    expect(c.position(1_000 + SMOOTH_MS / 2)).toBeCloseTo(1_200 + SMOOTH_MS / 2 - 100);
    expect(c.position(1_000 + SMOOTH_MS)).toBe(1_200 + SMOOTH_MS); // converged
  });

  it("never runs backwards when the backend lags behind", () => {
    const c = new PlaybackClock();
    c.sync(sync(0, true, true), 0);
    c.sync(sync(1_000 - HARD_THRESHOLD_MS), 1_000);
    let last = c.position(1_000);
    for (let t = 1_000; t <= 1_000 + SMOOTH_MS + 50; t += 16) {
      const p = c.position(t);
      expect(p).toBeGreaterThanOrEqual(last);
      last = p;
    }
  });

  it("jumps on large drift, seeks and play-state changes", () => {
    const c = new PlaybackClock();
    c.sync(sync(0, true, true), 0);
    c.sync(sync(60_000), 1_000);
    expect(c.position(1_000)).toBe(60_000);
    c.sync(sync(5_000, true, true), 2_000);
    expect(c.position(2_000)).toBe(5_000);
    c.sync(sync(5_100, false), 2_100);
    c.sync(sync(5_150, true), 9_000);
    expect(c.position(9_000)).toBe(5_150);
  });
});
