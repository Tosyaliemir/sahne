import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { PlaybackClock } from "../clock";
import type {
  PlaybackState,
  PlaybackStatus,
  PlayerSnapshot,
  PositionSource,
  PositionSync,
  TrackInfo,
} from "../types";

/** Reactive view of the backend player plus an interpolating clock. */
class PlayerStore {
  connected = $state(false);
  playerName = $state<string | null>(null);
  track = $state.raw<TrackInfo | null>(null);
  status = $state<PlaybackStatus>("stopped");
  source = $state<PositionSource>("player");
  /**
   * Interpolated position for text displays, refreshed a few times a second.
   * Animations read `positionNow()` in their own frame loops instead, so
   * nothing reactive changes on every frame.
   */
  positionMs = $state(0);

  readonly clock = new PlaybackClock();
  private sawEvent = false;
  private ticker: ReturnType<typeof setInterval> | undefined;

  async start(): Promise<() => void> {
    const unlisten: UnlistenFn[] = await Promise.all([
      listen<TrackInfo | null>("track-changed", (e) => this.onTrack(e.payload)),
      listen<PlaybackState>("playback-state", (e) => this.onPlayback(e.payload)),
      listen<PositionSync>("position-sync", (e) => this.onPosition(e.payload)),
      listen("player-disconnected", () => this.onDisconnected()),
    ]);

    // Events may have fired before the listeners existed; seed from a snapshot.
    try {
      const snap = await invoke<PlayerSnapshot>("get_state");
      if (!this.sawEvent) this.applySnapshot(snap);
    } catch (err) {
      console.error("get_state failed", err);
    }

    const refresh = () => {
      const pos = Math.floor(this.positionNow(performance.now()) / 250) * 250;
      if (pos !== this.positionMs) this.positionMs = pos;
    };
    refresh();
    this.ticker = setInterval(refresh, 250);

    return () => {
      clearInterval(this.ticker);
      unlisten.forEach((u) => u());
    };
  }

  /** Interpolated position at `now` (performance.now() scale), clamped to the track. */
  positionNow(now: number): number {
    const length = this.track?.length_ms ?? 0;
    const pos = this.clock.position(now);
    return length > 0 ? Math.min(pos, length) : pos;
  }

  private applySnapshot(snap: PlayerSnapshot): void {
    if (snap.playback) this.onPlayback(snap.playback);
    if (snap.track) this.onTrack(snap.track);
    if (snap.position) this.onPosition({ ...snap.position, hard: true });
  }

  private onTrack(track: TrackInfo | null): void {
    this.sawEvent = true;
    this.connected = true;
    this.track = track;
    if (track) this.playerName = track.player;
  }

  private onPlayback(state: PlaybackState): void {
    this.sawEvent = true;
    this.connected = true;
    this.playerName = state.player;
    this.status = state.status;
  }

  private onPosition(sync: PositionSync): void {
    this.sawEvent = true;
    this.source = sync.source;
    this.clock.sync(sync, performance.now());
  }

  private onDisconnected(): void {
    this.sawEvent = true;
    this.connected = false;
    this.playerName = null;
    this.track = null;
    this.status = "stopped";
    this.clock.sync({ position_ms: 0, playing: false, rate: 1, hard: true }, performance.now());
  }

  playPause = () => command("play_pause");
  next = () => command("next");
  previous = () => command("previous");
  seek = (positionMs: number) => command("seek", { positionMs: Math.round(positionMs) });
}

async function command(name: string, args?: Record<string, unknown>): Promise<void> {
  try {
    await invoke(name, args);
  } catch (err) {
    console.warn(`${name} failed`, err);
  }
}

export const player = new PlayerStore();
