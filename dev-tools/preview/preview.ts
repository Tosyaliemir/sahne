/**
 * Renders Sahne in a plain browser with a mocked Tauri backend, so the UI can
 * be checked (or screenshotted headlessly) without Spotify or a window.
 *
 *   pnpm vite --port 1430
 *   http://127.0.0.1:1430/preview.html?view=main&style=klasik&at=12500
 *
 * Parameters: view=main|mini, style=klasik|kinetik|sade, at=<ms position>,
 * panel=settings, lyrics=synced|plain|loading|not_found, paused=1, lang=en,
 * static=1 (no CSS transitions/animations, for stable screenshots; also run
 * Chrome with --force-prefers-reduced-motion so Svelte transitions are instant)
 */
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { LyricsLoaded, PaletteChanged, PlayerSnapshot, Settings, TrackInfo } from "../../src/lib/types";

const q = new URLSearchParams(location.search);
const view = q.get("view") === "mini" ? "mini" : "main";
const at = Number(q.get("at") ?? 12_500);
const paused = q.get("paused") === "1";

const track: TrackInfo = {
  id: "/preview/1",
  title: "Şımarık",
  artist: "Tarkan",
  album: "Ölürüm Sana",
  length_ms: 234_000,
  art_url: `${location.origin}/dev-tools/preview/cover.png`,
  player: "sahnemock",
};

const line = (s: number, e: number, text: string) => ({ start_ms: s, end_ms: e, text, words: null });
const lyricsData: LyricsLoaded = {
  track_id: track.id,
  state: (q.get("lyrics") as LyricsLoaded["state"]) ?? "synced",
  offset_ms: 0,
  plain: "Takmış koluna elin adamını\nBeni görünce şımarık şımarık\nGülüyor bana dönüp\nÇıldırıyorum",
  lines: [
    line(0, 9_000, ""),
    line(9_000, 12_000, "Takmış koluna elin adamını beni"),
    line(12_000, 15_400, "Görünce şımarık şımarık gülüyor"),
    line(15_400, 18_000, "Dönüp dönüp bakıyor ığdır çığlık"),
    line(18_000, 21_000, "Öldürecek beni İstanbul'un ışıkları"),
    line(21_000, 30_000, ""),
    line(30_000, 33_000, "Cik cik cik"),
  ],
};

const palette: PaletteChanged = {
  track_id: track.id,
  art_path: null,
  background_path: null,
  palette: {
    dominant: "#6a3c7e",
    vibrant: "#f0663f",
    muted: "#7a5c6a",
    dark_bg: "#1f1225",
    light_text: "#fbf3f8",
    accent: "#f7cf3a",
  },
};

const settings: Settings = {
  preferred_player: null,
  lyrics_style: (q.get("style") as Settings["lyrics_style"]) ?? "klasik",
  font: "embedded",
  font_scale: 1,
  backdrop_blur: 40,
  backdrop_dim: 0.35,
  global_offset_ms: 0,
  mini_position: null,
  start_fullscreen: false,
  lighting: { enabled: true, mode: "gradyan", devices: ["Klavye"] },
  language: q.get("lang") === "en" ? "en" : "tr",
};

const position = { position_ms: at, monotonic_ms: 0, playing: !paused, rate: 1, source: "player", hard: true } as const;
const snapshot: PlayerSnapshot = {
  track,
  playback: { status: paused ? "paused" : "playing", player: "sahnemock" },
  position,
};

if (q.get("static") === "1") {
  const style = document.createElement("style");
  style.textContent = "*,*::before,*::after{transition:none!important;animation:none!important}";
  document.head.append(style);
}

clearMocks();
mockWindows(view);
mockIPC(
  (cmd) => {
    switch (cmd) {
      case "get_state":
        return snapshot;
      case "get_lyrics":
        return lyricsData;
      case "get_palette":
        return palette;
      case "get_settings":
      case "save_settings":
        return settings;
      case "list_players":
        return ["sahnemock", "spotify", "vlc"];
      case "lighting_status":
        return { available: true, state: "connected", devices: [{ name: "Klavye", leds: 104 }, { name: "Fare", leds: 2 }] };
      default:
        return null;
    }
  },
  { shouldMockEvents: true },
);

await import("../../src/main");

// Re-send after mount so listeners that registered late still see the state.
setTimeout(() => {
  void emit("position-sync", position);
  void emit("palette-changed", palette);
  if (q.get("panel") === "settings") {
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "," }));
  }
}, 300);
