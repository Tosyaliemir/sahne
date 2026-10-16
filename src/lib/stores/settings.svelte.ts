import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Settings } from "../types";
import { resolveLang, setLang } from "../i18n/index.svelte";
import { ui } from "./ui.svelte";
import { frames } from "../frames";

const SAVE_DELAY_MS = 300;

export const DEFAULT_SETTINGS: Settings = {
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
  language: "system",
  backdrop_style: "fluid",
  edge_glow: false,
  screensaver_enabled: true,
  screensaver_minutes: 3,
  screensaver_keep_awake: false,
  cover_effect: "none",
  performance_mode: "auto",
  announce_lyrics: false,
};

class SettingsStore {
  current = $state.raw<Settings>(DEFAULT_SETTINGS);
  private saveTimer: ReturnType<typeof setTimeout> | undefined;
  /** Local edits not yet sent; only these fields go to the backend. */
  private pending: Partial<Settings> = {};

  async start(): Promise<() => void> {
    // Unsent local edits win over an incoming broadcast until they are sent.
    const unlisten = await listen<Settings>("settings-changed", (e) =>
      this.apply({ ...e.payload, ...this.pending }),
    );
    try {
      this.apply(await invoke<Settings>("get_settings"));
    } catch (err) {
      console.error("get_settings failed", err);
      this.apply(DEFAULT_SETTINGS);
    }
    return unlisten;
  }

  /**
   * Changes some settings locally right away and sends just those fields
   * shortly after. The backend merges them onto its current state, so a
   * stale copy here can't undo changes made by the other window or by the
   * backend itself (e.g. the mini window's position).
   */
  update(patch: Partial<Settings>): void {
    this.pending = { ...this.pending, ...patch };
    this.apply({ ...this.current, ...patch });
    clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => {
      const send = this.pending;
      this.pending = {};
      invoke<Settings>("update_settings", { patch: send }).catch((err) =>
        console.warn("update_settings failed", err),
      );
    }, SAVE_DELAY_MS);
  }

  private apply(s: Settings): void {
    this.current = s;
    frames.wake();
    ui.style = s.lyrics_style;
    const lang = resolveLang(s.language);
    setLang(lang);
    document.documentElement.lang = lang;
    const root = document.documentElement.style;
    root.setProperty("--lyrics-scale", String(s.font_scale));
    root.setProperty("--backdrop-blur", `${s.backdrop_blur}px`);
    root.setProperty("--backdrop-dim", String(s.backdrop_dim));
    root.setProperty(
      "--font-family",
      s.font === "system" ? "system-ui, sans-serif" : '"Manrope", system-ui, sans-serif',
    );
  }
}

export const settings = new SettingsStore();
