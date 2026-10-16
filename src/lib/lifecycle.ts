/**
 * Collects cleanup functions for a component, including ones that arrive
 * late from async setup (e.g. a store's `start()` awaiting `listen()`).
 * Cleanups that resolve after `dispose()` run immediately instead of
 * leaking listeners (seen with HMR and when the mini window closes).
 */
export class Lifecycle {
  private stops: (() => void)[] = [];
  private disposed = false;

  /** Adds a cleanup, or a promise of one. */
  add(stop: (() => void) | Promise<() => void>): void {
    if (typeof stop === "function") {
      this.keep(stop);
      return;
    }
    stop.then(
      (s) => this.keep(s),
      (err) => console.warn("setup failed", err),
    );
  }

  private keep(stop: () => void): void {
    if (this.disposed) stop();
    else this.stops.push(stop);
  }

  dispose(): void {
    this.disposed = true;
    for (const stop of this.stops.splice(0)) {
      try {
        stop();
      } catch (err) {
        console.warn("cleanup failed", err);
      }
    }
  }
}
