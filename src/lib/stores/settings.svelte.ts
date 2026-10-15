import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Settings } from "../types";
import { setLang } from "../i18n/index.svelte";
import { ui } from "./ui.svelte";

const SAVE_DELAY_MS = 300;

export const DEFAULT_SETTINGS: Settings = {
  preferred_player: null,
  lyrics_style: "klasik",
  font: "embedded",
  font_scale: 1,
  backdrop_blur: 0,
  backdrop_dim: 0.35,
  global_offset_ms: 0,
  mini_position: null,
  start_fullscreen: false,
  controls_hide_secs: 2,
  language: "tr",
  backdrop_style: "akiskan",
  edge_glow: false,
  screensaver_enabled: true,
  screensaver_minutes: 3,
  screensaver_keep_awake: false,
};

class SettingsStore {
  current = $state.raw<Settings>(DEFAULT_SETTINGS);
  private saveTimer: ReturnType<typeof setTimeout> | undefined;

  async start(): Promise<() => void> {
    const unlisten = await listen<Settings>("settings-changed", (e) => this.apply(e.payload));
    try {
      this.apply(await invoke<Settings>("get_settings"));
    } catch (err) {
      console.error("get_settings failed", err);
      this.apply(DEFAULT_SETTINGS);
    }
    return unlisten;
  }

  /** Changes some settings locally right away and saves them shortly after. */
  update(patch: Partial<Settings>): void {
    this.apply({ ...this.current, ...patch });
    clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => {
      invoke<Settings>("save_settings", { newSettings: this.current }).catch((err) =>
        console.warn("save_settings failed", err),
      );
    }, SAVE_DELAY_MS);
  }

  private apply(s: Settings): void {
    this.current = s;
    ui.style = s.lyrics_style;
    setLang(s.language);
    document.documentElement.lang = s.language;
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
