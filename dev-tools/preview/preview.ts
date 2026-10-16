/**
 * Renders Sahne in a plain browser with a mocked Tauri backend, so the UI can
 * be checked (or screenshotted headlessly) without Spotify or a window.
 *
 *   pnpm vite --port 1430
 *   http://127.0.0.1:1430/preview.html?view=main&style=classic&at=12500
 *
 * Parameters:
 *   view=main|mini
 *   style=classic|kinetic|minimal|wave|blend|wall
 *   at=<position in ms>, paused=1
 *   focus=1           lyrics focus (as after pressing L) instead of the split view
 *   panel=settings    open the settings panel
 *   lyrics=synced|plain|loading|not_found|instrumental|offline|error
 *   lang=en|tr
 *   bg=fluid|blurred|cover|color, glow=1, cover=none|metal|electric
 *   perf=auto|quality|battery
 *   bar=1             the bottom bar never hides
 *   static=1          no CSS transitions or animations, for stable screenshots
 *                     (also run Chrome with --force-prefers-reduced-motion so
 *                     Svelte transitions are instant)
 *   fixture=<name>    use dev-tools/preview/fixtures/<name>.json, made by
 *                     `pnpm preview:fixture` (git-ignored; holds real lyrics)
 */
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { LyricsLoaded, PaletteChanged, PlayerSnapshot, Settings, TrackInfo } from "../../src/lib/types";

const q = new URLSearchParams(location.search);
const view = q.get("view") === "mini" ? "mini" : "main";
const at = Number(q.get("at") ?? 12_500);
const paused = q.get("paused") === "1";

/** A fixture from `pnpm preview:fixture`: the app's own output for a real track. */
interface Fixture {
  track: TrackInfo;
  lyrics: LyricsLoaded;
  palette: PaletteChanged["palette"];
  cover: string;
  background: string;
}
const fixtureName = q.get("fixture");
const fixtureDir = "/dev-tools/preview/fixtures/";
const fixture: Fixture | null = fixtureName
  ? await fetch(`${fixtureDir}${encodeURIComponent(fixtureName)}.json`).then((r) =>
      r.ok
        ? (r.json() as Promise<Fixture>)
        : Promise.reject(new Error(`fixture ${fixtureName}: HTTP ${r.status}`)),
    )
  : null;

const track: TrackInfo = fixture?.track ?? {
  id: "/preview/1",
  title: "Işıklı Günler",
  artist: "Örnek Sanatçı",
  album: "Deneme Albümü",
  length_ms: 234_000,
  art_url: `${location.origin}/dev-tools/preview/cover.png`,
  player: "sahnemock",
};

const line = (s: number, e: number, text: string) => ({ start_ms: s, end_ms: e, text, words: null });
const lyricsData: LyricsLoaded = fixture
  ? {
      ...fixture.lyrics,
      track_id: track.id,
      state: (q.get("lyrics") as LyricsLoaded["state"]) ?? fixture.lyrics.state,
    }
  : {
      track_id: track.id,
      state: (q.get("lyrics") as LyricsLoaded["state"]) ?? "synced",
      offset_ms: 0,
      plain:
        "Çiçekler sabahı bekliyor\nGölgede üşüyen İstanbul\nIşıklar ağır ağır sönüyor\nŞarkı bitmeden dönüyoruz",
      lines: [
        line(0, 9_000, ""),
        line(9_000, 12_000, "Çiçekler sabahı bekliyor"),
        line(12_000, 15_400, "Gölgede üşüyen İstanbul"),
        line(15_400, 18_000, "Işıklar ağır ağır sönüyor"),
        line(18_000, 21_000, "Şarkı bitmeden dönüyoruz"),
        // Instrumental interlude.
        line(21_000, 30_000, ""),
        line(30_000, 33_000, "Öğle güneşi çığlık çığlığa"),
      ],
    };

const palette: PaletteChanged = fixture
  ? {
      track_id: track.id,
      // Served by Vite; convertFileSrc is mocked to pass paths through below.
      art_path: fixtureDir + fixture.cover,
      background_path: fixtureDir + fixture.background,
      palette: fixture.palette,
    }
  : {
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
        ambient: ["#f0663f", "#f7cf3a", "#8a4fb0", "#2f7fd0", "#d9467a"],
      },
    };

const settings: Settings = {
  preferred_player: null,
  lyrics_style: (q.get("style") as Settings["lyrics_style"]) ?? "classic",
  font: "embedded",
  font_scale: 1,
  backdrop_blur: 0,
  backdrop_dim: 0.35,
  global_offset_ms: 0,
  mini_position: null,
  start_fullscreen: false,
  controls_hide_secs: q.get("bar") === "1" ? 0 : 2,
  language: q.get("lang") === "tr" ? "tr" : "en",
  backdrop_style: (q.get("bg") as Settings["backdrop_style"]) ?? "fluid",
  edge_glow: q.get("glow") === "1",
  screensaver_enabled: true,
  screensaver_minutes: 3,
  screensaver_keep_awake: false,
  cover_effect: (q.get("cover") as Settings["cover_effect"]) ?? "none",
  // Headless Chrome renders in software; keep the fluid backdrop anyway.
  performance_mode: (q.get("perf") as Settings["performance_mode"]) ?? "quality",
};

const position = {
  position_ms: at,
  monotonic_ms: 0,
  playing: !paused,
  rate: 1,
  source: "player",
  hard: true,
} as const;
const snapshot: PlayerSnapshot = {
  track,
  playback: { status: paused ? "paused" : "playing", player: "sahnemock" },
  position,
  volume: 0.62,
};

if (q.get("static") === "1") {
  const style = document.createElement("style");
  style.textContent = "*,*::before,*::after{transition:none!important;animation:none!important}";
  document.head.append(style);
}

clearMocks();
mockWindows(view);
// The app loads cached images through convertFileSrc + fetch; in the preview
// the "cached files" are plain same-origin URLs.
(
  window as unknown as { __TAURI_INTERNALS__: { convertFileSrc: (p: string) => string } }
).__TAURI_INTERNALS__.convertFileSrc = (path) => path;
mockIPC(
  (cmd, args) => {
    switch (cmd) {
      case "get_state":
        return snapshot;
      case "get_lyrics":
        return lyricsData;
      case "get_palette":
        return palette;
      case "get_settings":
        return settings;
      case "update_settings":
        Object.assign(settings, (args as { patch?: Partial<Settings> } | undefined)?.patch ?? {});
        return settings;
      case "cache_size":
        return 19 * 1024 * 1024;
      case "clear_cache":
        return 0;
      case "list_players":
        return ["sahnemock", "spotify", "vlc"];
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
  if (q.get("focus") === "1") {
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "l" }));
  }
  if (q.get("panel") === "settings") {
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "," }));
  }
}, 300);
