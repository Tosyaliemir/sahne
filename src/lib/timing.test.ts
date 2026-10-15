import { describe, expect, it } from "vitest";
import {
  activeLineIndex,
  activeWordIndex,
  interludeAt,
  lyricTime,
  MAX_MS_PER_CHAR,
  wordProgress,
  wordTimings,
} from "./timing";
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

describe("wordTimings", () => {
  it("uses Enhanced LRC timings when present", () => {
    const words = [
      { start_ms: 1000, end_ms: 1400, text: "Hello" },
      { start_ms: 1400, end_ms: 2000, text: "there" },
    ];
    expect(wordTimings({ start_ms: 1000, end_ms: 3000, text: "Hello there", words })).toBe(words);
  });

  it("splits the line in proportion to word length", () => {
    // weights: "a"=2, "bbb"=4, "cc"=3 → total 9 over 900 ms (cap is 9*180 = 1620 ms)
    const w = wordTimings(line(1000, 1900, "a bbb cc"));
    expect(w.map((x) => [x.start_ms, x.end_ms, x.text])).toEqual([
      [1000, 1200, "a"],
      [1200, 1600, "bbb"],
      [1600, 1900, "cc"],
    ]);
  });

  it("does not stretch words across a long pause", () => {
    const w = wordTimings(line(0, 30_000, "hi you"));
    const total = (3 + 4) * MAX_MS_PER_CHAR;
    expect(w[w.length - 1].end_ms).toBe(total);
  });

  it("counts Turkish letters as single characters", () => {
    const w = wordTimings(line(0, 1000, "şı ab"));
    expect(w[0].end_ms).toBe(500);
  });

  it("is contiguous and ordered", () => {
    const w = wordTimings(line(5000, 8000, "We're no strangers to love"));
    for (let i = 1; i < w.length; i++) expect(w[i].start_ms).toBe(w[i - 1].end_ms);
    expect(w[0].start_ms).toBe(5000);
    expect(w[w.length - 1].end_ms).toBeLessThanOrEqual(8000);
  });

  it("handles blank lines", () => {
    expect(wordTimings(line(0, 1000, "   "))).toEqual([]);
  });
});

describe("word progress", () => {
  const w = { start_ms: 100, end_ms: 300, text: "x" };
  it("clamps and interpolates", () => {
    expect(wordProgress(w, 0)).toBe(0);
    expect(wordProgress(w, 200)).toBe(0.5);
    expect(wordProgress(w, 900)).toBe(1);
  });
  it("finds the active word", () => {
    const ws = wordTimings(line(0, 900, "a bbb cc"));
    expect(activeWordIndex(ws, -1)).toBe(-1);
    expect(activeWordIndex(ws, 0)).toBe(0);
    expect(activeWordIndex(ws, 250)).toBe(1);
    expect(activeWordIndex(ws, 5000)).toBe(2);
  });
});

describe("interludeAt", () => {
  const lines = [line(8000, 10_000, "first"), line(10_000, 20_000, ""), line(20_000, 22_000, "second"), line(22_000, 23_000, "")];
  it("detects a long intro", () => {
    expect(interludeAt(lines, 4000)).toEqual({ start_ms: 0, end_ms: 8000, progress: 0.5 });
  });
  it("detects a long break line", () => {
    expect(interludeAt(lines, 15_000)?.progress).toBe(0.5);
  });
  it("ignores short breaks and sung lines", () => {
    expect(interludeAt(lines, 9000)).toBeNull();
    expect(interludeAt(lines, 22_500)).toBeNull();
    expect(interludeAt([line(1000, 2000, "x")], 500)).toBeNull();
  });
});
