import { describe, expect, it } from "vitest";
import { registerAt, registerIn } from "./registry";

const el = () => ({}) as HTMLElement;

describe("registerAt", () => {
  it("fills and clears its slot", () => {
    const list: (HTMLElement | undefined)[] = [];
    const a = el();
    const cleanup = registerAt(list, 2)(a);
    expect(list[2]).toBe(a);
    cleanup();
    expect(list[2]).toBeUndefined();
  });

  it("does not clear a slot another element took over", () => {
    const list: (HTMLElement | undefined)[] = [];
    const a = el();
    const b = el();
    const cleanA = registerAt(list, 0)(a);
    registerAt(list, 0)(b);
    cleanA();
    expect(list[0]).toBe(b);
  });
});

describe("registerIn", () => {
  it("fills and clears grid cells", () => {
    const grid: HTMLElement[][] = [];
    const a = el();
    const b = el();
    const cleanA = registerIn(grid, 1, 0)(a);
    registerIn(grid, 1, 1)(b);
    expect(grid[1]).toEqual([a, b]);
    cleanA();
    expect(grid[1][0]).toBeUndefined();
    expect(grid[1][1]).toBe(b);
  });
});
