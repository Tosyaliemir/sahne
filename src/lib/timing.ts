import type { Line, Word } from "./types";

/**
 * Index of the line being sung at `t` (ms, lyric time), or -1 before the
 * first line. Lines are sorted by start time.
 */
export function activeLineIndex(lines: readonly Line[], t: number): number {
  let lo = 0;
  let hi = lines.length - 1;
  let found = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid].start_ms <= t) {
      found = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return found;
}

/** Converts playback position to lyric time using the user's offset. */
export function lyricTime(positionMs: number, offsetMs: number): number {
  return positionMs - offsetMs;
}

/**
 * Upper bound on how long one character is sung. A line followed by a long
 * pause would otherwise stretch its words across the silence.
 */
export const MAX_MS_PER_CHAR = 180;
/** Minimum singing time for a line, so very short lines still animate. */
export const MIN_LINE_SING_MS = 600;
/** Gaps at least this long show the interlude indicator. */
export const INTERLUDE_MIN_MS = 5000;

/**
 * Word timings for a line: the real ones from Enhanced LRC, or the line's
 * singing time split across words in proportion to their length.
 */
export function wordTimings(line: Line): Word[] {
  if (line.words && line.words.length > 0) return line.words;
  const texts = line.text.split(/\s+/).filter(Boolean);
  if (texts.length === 0) return [];

  // Each word also "owns" the gap after it, so short words still get time.
  const weights = texts.map((w) => [...w].length + 1);
  const total = weights.reduce((a, b) => a + b, 0);
  const lineMs = Math.max(0, line.end_ms - line.start_ms);
  const singMs = Math.min(lineMs, Math.max(MIN_LINE_SING_MS, total * MAX_MS_PER_CHAR));

  const words: Word[] = [];
  let t = line.start_ms;
  texts.forEach((text, i) => {
    const d = (singMs * weights[i]) / total;
    words.push({ start_ms: Math.round(t), end_ms: Math.round(t + d), text });
    t += d;
  });
  return words;
}

/** 0 before the word starts, 1 once it is over, linear in between. */
export function wordProgress(word: Word, t: number): number {
  if (t <= word.start_ms) return 0;
  if (t >= word.end_ms) return 1;
  return (t - word.start_ms) / (word.end_ms - word.start_ms);
}

/** Index of the word being sung (last started word), or -1. */
export function activeWordIndex(words: readonly Word[], t: number): number {
  let found = -1;
  for (let i = 0; i < words.length && words[i].start_ms <= t; i++) found = i;
  return found;
}

export interface Interlude {
  start_ms: number;
  end_ms: number;
  /** 0..1 through the break. */
  progress: number;
}

/**
 * The instrumental break at `t`, if any: a long empty line, or a long intro
 * before the first line.
 */
export function interludeAt(lines: readonly Line[], t: number): Interlude | null {
  const make = (start: number, end: number): Interlude | null =>
    end - start >= INTERLUDE_MIN_MS
      ? { start_ms: start, end_ms: end, progress: Math.min(1, Math.max(0, (t - start) / (end - start))) }
      : null;
  if (lines.length === 0) return null;
  const i = activeLineIndex(lines, t);
  if (i === -1) return make(0, lines[0].start_ms);
  const line = lines[i];
  return line.text.trim() === "" ? make(line.start_ms, line.end_ms) : null;
}

/** Words per second at which a line counts as a fast (rap/flex) passage. */
export const FAST_WORDS_PER_SEC = 3.4;
/** Fast passages shorter than this stay classic, so one quick line doesn't flicker. */
export const FAST_MIN_SECTION_MS = 3500;

/** How quickly a line's words go by, from its (real or estimated) word timings. */
export function wordsPerSecond(line: Line): number {
  const ws = wordTimings(line);
  if (ws.length < 4) return 0;
  const span = ws[ws.length - 1].end_ms - ws[0].start_ms;
  return span > 0 ? (ws.length * 1000) / span : 0;
}

/**
 * Blend mode: for each line, whether it belongs to a fast passage that
 * should switch to the kinetic style. Neighbouring fast lines are merged, and
 * passages shorter than FAST_MIN_SECTION_MS are ignored.
 */
export function blendPlan(lines: readonly Line[]): boolean[] {
  const fast = lines.map((l) => wordsPerSecond(l) >= FAST_WORDS_PER_SEC);
  const plan = new Array<boolean>(lines.length).fill(false);
  let i = 0;
  while (i < lines.length) {
    if (!fast[i]) {
      i++;
      continue;
    }
    let j = i;
    while (j + 1 < lines.length && fast[j + 1]) j++;
    if (lines[j].end_ms - lines[i].start_ms >= FAST_MIN_SECTION_MS) {
      for (let k = i; k <= j; k++) plan[k] = true;
    }
    i = j + 1;
  }
  return plan;
}
