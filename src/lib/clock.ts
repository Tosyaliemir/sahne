/**
 * Interpolates playback position between backend syncs (~1 Hz) so the UI can
 * advance smoothly every frame.
 *
 * Small disagreements (≤ HARD_THRESHOLD_MS) are blended out over SMOOTH_MS
 * instead of jumping. The blend speed is bounded so the position never runs
 * backwards while playing.
 */
export const HARD_THRESHOLD_MS = 300;
export const SMOOTH_MS = 500;

export interface ClockSync {
  position_ms: number;
  playing: boolean;
  rate: number;
  hard: boolean;
}

export class PlaybackClock {
  private anchorPos = 0;
  private anchorTime = 0;
  private playing = false;
  private rate = 1;
  /** Offset still to be blended out, and when blending started. */
  private drift = 0;
  private driftStart = 0;

  /** Position in ms at time `now` (performance.now() scale). */
  position(now: number): number {
    const elapsed = this.playing ? (now - this.anchorTime) * this.rate : 0;
    let pos = this.anchorPos + elapsed;
    if (this.drift !== 0) {
      const t = (now - this.driftStart) / SMOOTH_MS;
      if (t >= 1) this.drift = 0;
      else pos += this.drift * (1 - t);
    }
    return Math.max(0, pos);
  }

  isPlaying(): boolean {
    return this.playing;
  }

  sync(s: ClockSync, now: number): void {
    const predicted = this.position(now);
    const diff = s.position_ms - predicted;
    const soft =
      !s.hard && s.playing && this.playing && s.rate === this.rate && Math.abs(diff) <= HARD_THRESHOLD_MS;

    this.anchorPos = s.position_ms;
    this.anchorTime = now;
    this.playing = s.playing;
    this.rate = s.rate;
    if (soft) {
      // Start from where we were displaying and converge onto the new truth.
      this.drift = -diff;
      this.driftStart = now;
    } else {
      this.drift = 0;
    }
  }
}
