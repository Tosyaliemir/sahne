export const STYLES = ["klasik", "kinetik", "sade", "dalga", "harman", "duvar"] as const;
export type LyricsStyle = (typeof STYLES)[number];

const reducedMotion =
  typeof matchMedia === "function" ? matchMedia("(prefers-reduced-motion: reduce)") : null;

const SWITCH_FADE_MS = 220;

class UiStore {
  /** Style chosen by the user; the settings store keeps it in sync. */
  style = $state<LyricsStyle>("klasik");
  reducedMotion = $state(reducedMotion?.matches ?? false);
  controlsVisible = $state(true);
  settingsOpen = $state(false);
  /** The window is hidden or minimised; nothing should animate. */
  windowHidden = $state(false);
  /** The backend put us on screen because the computer went idle. */
  screensaver = $state(false);
  /**
   * Lyrics fill the screen (true) or share it with the cover (false, the
   * default at startup). Not persisted on purpose.
   */
  lyricsFocus = $state(false);
  /** The lyrics pane is faded out while the layout switches under it. */
  switching = $state(false);
  private switchTimer: ReturnType<typeof setTimeout> | undefined;

  constructor() {
    reducedMotion?.addEventListener("change", (e) => (this.reducedMotion = e.matches));
  }

  /** What is actually rendered: reduced motion always wins. */
  get effectiveStyle(): LyricsStyle {
    return this.reducedMotion ? "sade" : this.style;
  }

  /**
   * Fades the lyrics out, swaps the layout while they are invisible, then
   * fades them back in. Resizing the pane mid-animation would make the line
   * layouts re-measure every frame and jitter.
   */
  setLyricsFocus(focus: boolean): void {
    if (focus === this.lyricsFocus) return;
    clearTimeout(this.switchTimer);
    if (this.reducedMotion) {
      this.lyricsFocus = focus;
      queueMicrotask(() => window.dispatchEvent(new Event("resize")));
      return;
    }
    this.switching = true;
    this.switchTimer = setTimeout(() => {
      this.lyricsFocus = focus;
      // The views lay their lines out on window resize; the pane changed size.
      requestAnimationFrame(() => {
        window.dispatchEvent(new Event("resize"));
        this.switching = false;
      });
    }, SWITCH_FADE_MS);
  }

  nextStyle(): LyricsStyle {
    return STYLES[(STYLES.indexOf(this.style) + 1) % STYLES.length];
  }
}

export const ui = new UiStore();
