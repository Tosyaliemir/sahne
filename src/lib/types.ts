// Mirrors src-tauri/src/events.rs; the Rust `ts_contract` test pins the wire format.

export interface TrackInfo {
  id: string;
  title: string;
  artist: string;
  album: string;
  length_ms: number;
  art_url: string | null;
  player: string;
}

export type PlaybackStatus = "playing" | "paused" | "stopped";

export interface PlaybackState {
  status: PlaybackStatus;
  player: string;
}

export type PositionSource = "player" | "estimated";

export interface PositionSync {
  position_ms: number;
  monotonic_ms: number;
  playing: boolean;
  rate: number;
  source: PositionSource;
  hard: boolean;
}

export interface PlayerSnapshot {
  track: TrackInfo | null;
  playback: PlaybackState | null;
  position: PositionSync | null;
}
