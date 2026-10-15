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

export interface Word {
  start_ms: number;
  end_ms: number;
  text: string;
}

export interface Line {
  start_ms: number;
  end_ms: number;
  /** Empty for an instrumental break. */
  text: string;
  words: Word[] | null;
}

export type LyricsState =
  | "loading"
  | "synced"
  | "plain"
  | "instrumental"
  | "not_found"
  | "offline"
  | "error";

export interface LyricsLoaded {
  track_id: string;
  state: LyricsState;
  lines: Line[];
  plain: string | null;
  /** Per-track correction; positive shows lyrics later. */
  offset_ms: number;
}

export interface Palette {
  dominant: string;
  vibrant: string;
  muted: string;
  dark_bg: string;
  light_text: string;
  accent: string;
  /** Five distinct backdrop colors, most characteristic first. */
  ambient: string[];
}

export interface PaletteChanged {
  track_id: string;
  art_path: string | null;
  background_path: string | null;
  palette: Palette;
}

export type LyricsStyleSetting = "klasik" | "kinetik" | "sade" | "dalga" | "harman";

export interface Settings {
  preferred_player: string | null;
  lyrics_style: LyricsStyleSetting;
  font: "embedded" | "system";
  font_scale: number;
  backdrop_blur: number;
  backdrop_dim: number;
  global_offset_ms: number;
  mini_position: [number, number] | null;
  start_fullscreen: boolean;
  language: "tr" | "en";
  backdrop_style: "akiskan" | "bulanik";
  screensaver_enabled: boolean;
  screensaver_minutes: number;
  screensaver_keep_awake: boolean;
}
