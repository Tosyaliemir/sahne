import { describe, expect, it } from "vitest";
import { FrameScheduler } from "./frames";

/** Drives the scheduler by hand instead of requestAnimationFrame. */
function manual() {
  let queued: FrameRequestCallback | null = null;
  let now = 0;
  const scheduler = new FrameScheduler(
    (cb) => {
      queued = cb;
      return 1;
    },
    () => {
      queued = null;
    },
  );
  const step = () => {
    const cb = queued;
    queued = null;
    now += 16;
    cb?.(now);
    return cb !== null;
  };
  return { scheduler, step, isQueued: () => queued !== null };
}

describe("FrameScheduler (P1)", () => {
  it("stops drawing when nobody needs frames, draws once on wake", () => {
    const { scheduler, step, isQueued } = manual();
    let playing = true;
    let ticks = 0;
    scheduler.add({ tick: () => ticks++, needsFrame: () => playing });
    for (let i = 0; i < 5; i++) step();
    expect(ticks).toBe(5);
    playing = false; // paused
    step(); // the frame already requested runs once more
    expect(isQueued()).toBe(false);
    const before = ticks;
    expect(step()).toBe(false); // nothing scheduled: zero frames while paused
    expect(ticks).toBe(before);
    scheduler.wake(); // e.g. a seek while paused
    expect(step()).toBe(true);
    expect(ticks).toBe(before + 1);
    expect(isQueued()).toBe(false);
  });

  it("requests no frames while hidden and resumes when shown", () => {
    const { scheduler, step, isQueued } = manual();
    let ticks = 0;
    scheduler.add({ tick: () => ticks++, needsFrame: () => true });
    scheduler.setHidden(true);
    expect(isQueued()).toBe(false);
    expect(step()).toBe(false);
    scheduler.setHidden(false);
    expect(step()).toBe(true);
    expect(ticks).toBe(1);
  });

  it("observers see frames without causing them", () => {
    const { scheduler, step, isQueued } = manual();
    let seen = 0;
    scheduler.observe(() => seen++);
    expect(isQueued()).toBe(false);
    scheduler.add({ tick: () => {}, needsFrame: () => false });
    step();
    expect(seen).toBe(1);
    expect(isQueued()).toBe(false);
  });
});
