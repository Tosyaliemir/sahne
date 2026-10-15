import type { Line } from "./types";

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
