export type Level = "error" | "warn" | "info";

/**
 * Rate-limits and de-duplicates log messages: at most `max` per `windowMs`;
 * an identical message repeated within the window is counted, and when the
 * window ends a summary line reports what was dropped or repeated.
 */
export class LogLimiter {
  private sent = 0;
  private dropped = 0;
  private repeats = new Map<string, { level: Level; count: number }>();
  private timer: ReturnType<typeof setTimeout> | undefined;

  constructor(
    private send: (level: Level, message: string) => void,
    private max = 20,
    private windowMs = 10_000,
  ) {}

  log(level: Level, message: string): void {
    this.timer ??= setTimeout(() => this.flush(), this.windowMs);
    const seen = this.repeats.get(message);
    if (seen) {
      seen.count++;
      return;
    }
    this.repeats.set(message, { level, count: 0 });
    if (this.sent >= this.max) {
      this.dropped++;
      return;
    }
    this.sent++;
    this.send(level, message);
  }

  /** Ends the current window (also called by its timer). */
  flush(): void {
    clearTimeout(this.timer);
    this.timer = undefined;
    for (const [message, r] of this.repeats) {
      if (r.count > 0) this.send(r.level, `(repeated ${r.count} more times) ${message}`);
    }
    if (this.dropped > 0) this.send("warn", `${this.dropped} further log messages dropped`);
    this.sent = 0;
    this.dropped = 0;
    this.repeats.clear();
  }
}
