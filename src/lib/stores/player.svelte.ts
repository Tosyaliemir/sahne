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

/** Reactive view of the backend player, plus a per-frame interpolated position. */
class PlayerStore {
  connected = $state(false);
  playerName = $state<string | null>(null);
  track = $state<TrackInfo | null>(null);
  status = $state<PlaybackStatus>("stopped");
  source = $state<PositionSource>("player");
  /** Interpolated position, refreshed every animation frame. */
  positionMs = $state(0);

  readonly clock = new PlaybackClock();
  private sawEvent = false;
  private frame = 0;

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

    const loop = (now: number) => {
      const length = this.track?.length_ms ?? 0;
      const pos = this.clock.position(now);
      this.positionMs = length > 0 ? Math.min(pos, length) : pos;
      this.frame = requestAnimationFrame(loop);
    };
    this.frame = requestAnimationFrame(loop);

    return () => {
      cancelAnimationFrame(this.frame);
      unlisten.forEach((u) => u());
    };
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
