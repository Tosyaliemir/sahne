export const STYLES = ["klasik", "kinetik", "sade"] as const;
export type LyricsStyle = (typeof STYLES)[number];

const reducedMotion =
  typeof matchMedia === "function" ? matchMedia("(prefers-reduced-motion: reduce)") : null;

class UiStore {
  /** Style chosen by the user (persisted by the settings store). */
  style = $state<LyricsStyle>("klasik");
  reducedMotion = $state(reducedMotion?.matches ?? false);
  controlsVisible = $state(true);
  settingsOpen = $state(false);

  constructor() {
    reducedMotion?.addEventListener("change", (e) => (this.reducedMotion = e.matches));
  }

  /** What is actually rendered: reduced motion always wins. */
  get effectiveStyle(): LyricsStyle {
    return this.reducedMotion ? "sade" : this.style;
  }

  cycleStyle(): LyricsStyle {
    this.style = STYLES[(STYLES.indexOf(this.style) + 1) % STYLES.length];
    return this.style;
  }
}

export const ui = new UiStore();
