import { describe, expect, it } from "vitest";
import { activeLineIndex, lyricTime } from "./timing";
import type { Line } from "./types";

const line = (start_ms: number, end_ms: number, text = "x"): Line => ({ start_ms, end_ms, text, words: null });

describe("activeLineIndex", () => {
  const lines = [line(1000, 2000), line(2000, 5000), line(5000, 9000)];

  it("is -1 before the first line", () => {
    expect(activeLineIndex(lines, 0)).toBe(-1);
    expect(activeLineIndex(lines, 999)).toBe(-1);
  });

  it("switches exactly at line starts", () => {
    expect(activeLineIndex(lines, 1000)).toBe(0);
    expect(activeLineIndex(lines, 1999)).toBe(0);
    expect(activeLineIndex(lines, 2000)).toBe(1);
    expect(activeLineIndex(lines, 5000)).toBe(2);
  });

  it("stays on the last line after the end", () => {
    expect(activeLineIndex(lines, 60_000)).toBe(2);
  });

  it("handles empty input", () => {
    expect(activeLineIndex([], 1000)).toBe(-1);
  });
});

describe("lyricTime", () => {
  it("positive offset delays lyrics", () => {
    expect(lyricTime(10_000, 200)).toBe(9_800);
    expect(lyricTime(10_000, -200)).toBe(10_200);
  });
});
