/**
 * A mocked Tauri backend for component tests: answers the app's commands
 * from a fixture state and records every call. Each `renderApp()` imports
 * the app afresh, so the store singletons start clean.
 */
import { vi } from "vitest";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { LyricsLoaded, PaletteChanged, PlayerSnapshot, Settings, TrackInfo } from "../lib/types";

export const track: TrackInfo = {
  id: "/test/1",
  title: "Işıklı Günler",
  artist: "Örnek Sanatçı",
  album: "Deneme Albümü",
  length_ms: 200_000,
  art_url: null,
  player: "sahnemock",
};

const line = (start_ms: number, end_ms: number, text: string) => ({ start_ms, end_ms, text, words: null });

export function lyricsIn(state: LyricsLoaded["state"]): LyricsLoaded {
  return {
    track_id: track.id,
    state,
    offset_ms: 0,
    plain: state === "plain" ? "Çiçekler sabahı bekliyor\nGölgede üşüyen İstanbul" : null,
    lines:
      state === "synced"
        ? [
            line(1_000, 4_000, "Çiçekler sabahı bekliyor"),
            line(4_000, 7_000, "Gölgede üşüyen İstanbul"),
            line(7_000, 10_000, "Işıklar ağır ağır sönüyor"),
          ]
        : [],
  };
}

export const defaultSettings: Settings = {
  preferred_player: null,
  lyrics_style: "classic",
  font: "embedded",
  font_scale: 1,
  backdrop_blur: 0,
  backdrop_dim: 0.35,
  global_offset_ms: 0,
  mini_position: null,
  start_fullscreen: false,
  controls_hide_secs: 2,
  language: "en",
  // No WebGL in happy-dom; keep the CSS backdrop.
  backdrop_style: "blurred",
  edge_glow: false,
  screensaver_enabled: true,
  screensaver_minutes: 3,
  screensaver_keep_awake: false,
  cover_effect: "none",
  performance_mode: "auto",
};

export interface BackendState {
  snapshot: PlayerSnapshot | null;
  lyrics: LyricsLoaded | null;
  settings: Settings;
}

export interface Call {
  cmd: string;
  args: Record<string, unknown> | undefined;
}

export function playing(status: "playing" | "paused" = "playing", at = 2_000): PlayerSnapshot {
  return {
    track,
    playback: { status, player: "sahnemock" },
    position: {
      position_ms: at,
      monotonic_ms: 0,
      playing: status === "playing",
      rate: 1,
      source: "player",
      hard: true,
    },
    volume: 0.5,
  };
}

/** Installs the mock backend; returns the recorded calls. */
export function mockBackend(state: Partial<BackendState> = {}): Call[] {
  const s: BackendState = {
    snapshot: playing(),
    lyrics: lyricsIn("synced"),
    settings: { ...defaultSettings },
    ...state,
  };
  const calls: Call[] = [];
  const palette: PaletteChanged | null = null;
  clearMocks();
  mockWindows("main");
  mockIPC(
    (cmd, args) => {
      calls.push({ cmd, args: args as Record<string, unknown> | undefined });
      switch (cmd) {
        case "get_state":
          return s.snapshot;
        case "get_lyrics":
          return s.lyrics;
        case "get_palette":
          return palette;
        case "get_settings":
          return s.settings;
        case "update_settings":
          Object.assign(s.settings, (args as { patch: Partial<Settings> }).patch);
          return s.settings;
        case "list_players":
          return ["sahnemock", "spotify"];
        case "cache_size":
          return 3 * 1024 * 1024;
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
  return calls;
}

/** Renders a fresh copy of the main window. */
export async function renderApp() {
  vi.resetModules();
  const [{ render, cleanup }, { default: App }] = await Promise.all([
    import("@testing-library/svelte"),
    import("../App.svelte"),
  ]);
  const result = render(App);
  return { ...result, cleanup };
}

/** Lets pending promises and Svelte updates settle. */
export async function settle(ms = 20): Promise<void> {
  await new Promise((r) => setTimeout(r, ms));
}

export const commands = (calls: Call[], cmd: string) => calls.filter((c) => c.cmd === cmd);
