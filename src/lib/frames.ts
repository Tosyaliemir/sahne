/**
 * One shared animation-frame loop for the whole window (P1).
 *
 * Components register a client that says, each frame, whether it still needs
 * frames. The loop runs only while at least one client does, so a paused
 * player or a static screen costs nothing: when something changes (play,
 * pause, seek, a new track or palette), `wake()` gives every client one more
 * frame to draw its final state, and the loop stops again.
 *
 * While the window is hidden or minimised, no frames are requested at all.
 */
export interface FrameClient {
  /** Called on frames where the client is drawn. */
  tick(now: number): void;
  /** True while the client is animating and wants the next frame too. */
  needsFrame(): boolean;
}

interface Entry {
  client: FrameClient;
  /** Draw once on the next frame even if `needsFrame()` is false. */
  pending: boolean;
}

export class FrameScheduler {
  private entries = new Set<Entry>();
  private observers = new Set<(now: number) => void>();
  private raf = 0;
  private hidden = false;
  /** Frames actually run; read by the dev frame stats. */
  frames = 0;

  constructor(
    private request: (cb: FrameRequestCallback) => number = (cb) => requestAnimationFrame(cb),
    private cancel: (id: number) => void = (id) => cancelAnimationFrame(id),
  ) {}

  /** Registers a client (it is drawn once right away); returns its removal. */
  add(client: FrameClient): () => void {
    const entry: Entry = { client, pending: true };
    this.entries.add(entry);
    this.schedule();
    return () => {
      this.entries.delete(entry);
    };
  }

  /** Calls `cb` on every frame that runs anyway, without causing frames. */
  observe(cb: (now: number) => void): () => void {
    this.observers.add(cb);
    return () => {
      this.observers.delete(cb);
    };
  }

  /** Something visible changed: every client draws one more frame. */
  wake(): void {
    for (const e of this.entries) e.pending = true;
    this.schedule();
  }

  /** The window was hidden/minimised (true) or shown again (false). */
  setHidden(hidden: boolean): void {
    if (hidden === this.hidden) return;
    this.hidden = hidden;
    if (hidden) {
      this.cancel(this.raf);
      this.raf = 0;
    } else {
      this.wake();
    }
  }

  get isHidden(): boolean {
    return this.hidden;
  }

  /** Whether a frame is scheduled; false means the window is idle. */
  get running(): boolean {
    return this.raf !== 0;
  }

  private schedule(): void {
    if (this.raf || this.hidden || this.entries.size === 0) return;
    this.raf = this.request(this.frame);
  }

  private frame = (now: number): void => {
    this.raf = 0;
    this.frames++;
    let again = false;
    for (const e of this.entries) {
      if (e.pending || e.client.needsFrame()) {
        e.pending = false;
        e.client.tick(now);
      }
      if (e.client.needsFrame()) again = true;
    }
    for (const cb of this.observers) cb(now);
    if (again) this.schedule();
  };
}

export const frames = new FrameScheduler();

/**
 * Hooks the scheduler to page visibility, and returns a cleanup. The Tauri
 * side reports minimise/hide through `document.visibilityState` on both
 * WebKitGTK and WebView2.
 */
export function trackVisibility(scheduler: FrameScheduler = frames): () => void {
  const update = () => scheduler.setHidden(document.visibilityState === "hidden");
  update();
  document.addEventListener("visibilitychange", update);
  return () => document.removeEventListener("visibilitychange", update);
}
