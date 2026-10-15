export const STYLES = ["klasik", "kinetik", "sade"] as const;
export type LyricsStyle = (typeof STYLES)[number];

const reducedMotion =
  typeof matchMedia === "function" ? matchMedia("(prefers-reduced-motion: reduce)") : null;

class UiStore {
  /** Style chosen by the user; the settings store keeps it in sync. */
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

  nextStyle(): LyricsStyle {
    return STYLES[(STYLES.indexOf(this.style) + 1) % STYLES.length];
  }
}

export const ui = new UiStore();
