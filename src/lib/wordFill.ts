import type { Word } from "./types";
import { activeWordIndex, wordProgress } from "./timing";

/**
 * Per-frame word fill for one line of FillWord elements: sets each word's
 * --p (0..1) only when it changes, marks words as `on` once started and
 * `done` once finished, and `glow` on the word being sung right now.
 */
export class LineFiller {
  private last: number[] = [];
  private singing = -1;

  update(els: (HTMLElement | undefined)[] | undefined, words: readonly Word[], t: number): void {
    if (!els) return;
    for (let j = 0; j < words.length; j++) {
      const el = els[j];
      if (!el) continue;
      const p = Math.round(wordProgress(words[j], t) * 1000) / 1000;
      if (p !== this.last[j]) {
        el.style.setProperty("--p", String(p));
        if (p > 0 && !(this.last[j] > 0)) el.classList.add("on");
        if (p >= 1 && !(this.last[j] >= 1)) el.classList.add("done");
        this.last[j] = p;
      }
    }
    const aw = activeWordIndex(words, t);
    const singing = aw >= 0 && t < words[aw].end_ms ? aw : -1;
    if (singing !== this.singing) {
      if (this.singing >= 0) els[this.singing]?.classList.remove("glow");
      if (singing >= 0) els[singing]?.classList.add("glow");
      this.singing = singing;
    }
  }

  /** Clears a line that stopped being active. */
  reset(els: (HTMLElement | undefined)[] | undefined): void {
    for (const el of els ?? []) {
      el?.style.removeProperty("--p");
      el?.classList.remove("on", "done", "glow");
    }
    this.last = [];
    this.singing = -1;
  }
}
