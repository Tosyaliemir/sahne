import { invoke } from "@tauri-apps/api/core";
import { frames } from "../frames";
import { listen } from "@tauri-apps/api/event";
import type { LyricsLoaded } from "../types";
import { player } from "./player.svelte";
import { settings } from "./settings.svelte";

export const OFFSET_STEP_MS = 100;

class LyricsStore {
  current = $state.raw<LyricsLoaded | null>(null);

  /** Lyrics only count when they belong to the track that is playing. */
  get forTrack(): LyricsLoaded | null {
    const c = this.current;
    return c && player.track && c.track_id === player.track.id ? c : null;
  }

  /** Track offset plus the user's global offset; positive shows lyrics later. */
  get totalOffsetMs(): number {
    return (this.forTrack?.offset_ms ?? 0) + settings.current.global_offset_ms;
  }

  async start(): Promise<() => void> {
    const unlisten = await listen<LyricsLoaded>("lyrics-loaded", (e) => {
      this.current = e.payload;
      frames.wake();
    });
    try {
      const snap = await invoke<LyricsLoaded | null>("get_lyrics");
      if (snap && !this.current) this.current = snap;
    } catch (err) {
      console.error("get_lyrics failed", err);
    }
    return unlisten;
  }

  /** Nudges the current track's offset by `deltaMs` and persists it. */
  nudge(deltaMs: number): void {
    const c = this.forTrack;
    if (!c) return;
    const offset_ms = c.offset_ms + deltaMs;
    this.current = { ...c, offset_ms };
    frames.wake();
    invoke("set_lyrics_offset", { trackId: c.track_id, offsetMs: offset_ms }).catch((err) =>
      console.warn("set_lyrics_offset failed", err),
    );
  }

  retry(): void {
    invoke("retry_lyrics").catch((err) => console.warn("retry_lyrics failed", err));
  }
}

export const lyrics = new LyricsStore();
