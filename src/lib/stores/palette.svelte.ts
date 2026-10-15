import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Palette, PaletteChanged } from "../types";
import { player } from "./player.svelte";

const NEUTRAL: Palette = {
  dominant: "#5c5c66",
  vibrant: "#8a8a96",
  muted: "#6b6b75",
  dark_bg: "#121216",
  light_text: "#f4f2ee",
  accent: "#b8b8c4",
  ambient: ["#5c5c66", "#6f6f7a", "#4d4d57", "#7d7d88", "#585862"],
};

const ROLES = ["dominant", "vibrant", "muted", "dark_bg", "light_text", "accent"] as const;

/**
 * WebKitGTK refuses `<img src="asset://…">` while `fetch` of the same URL works,
 * so cached images are loaded into blob URLs.
 */
class BlobCache {
  private urls = new Map<string, string>();
  private pending = new Map<string, Promise<string | null>>();

  get(path: string): string | undefined {
    return this.urls.get(path);
  }

  load(path: string): Promise<string | null> {
    const ready = this.urls.get(path);
    if (ready) return Promise.resolve(ready);
    let p = this.pending.get(path);
    if (!p) {
      p = fetch(convertFileSrc(path))
        .then((r) => (r.ok ? r.blob() : Promise.reject(new Error(`HTTP ${r.status}`))))
        .then((blob) => {
          const url = URL.createObjectURL(blob);
          this.urls.set(path, url);
          return url;
        })
        .catch((err) => {
          console.warn("could not load cached image", path, String(err));
          return null;
        })
        .finally(() => this.pending.delete(path));
      this.pending.set(path, p);
    }
    return p;
  }

  /** Frees everything except `keep`. */
  prune(keep: (string | null | undefined)[]): void {
    for (const [path, url] of this.urls) {
      if (!keep.includes(path)) {
        URL.revokeObjectURL(url);
        this.urls.delete(path);
      }
    }
  }
}

class PaletteStore {
  current = $state.raw<PaletteChanged | null>(null);
  /** Blob URLs for the current artwork, filled in once loaded. */
  private artUrl = $state<string | null>(null);
  private backgroundUrl = $state<string | null>(null);
  private blobs = new BlobCache();

  private get forTrack(): PaletteChanged | null {
    const c = this.current;
    return c && player.track && c.track_id === player.track.id ? c : null;
  }

  get palette(): Palette {
    return this.forTrack?.palette ?? NEUTRAL;
  }

  /** Cached artwork if ready, otherwise the player's own URL. */
  get artSrc(): string | null {
    if (this.forTrack?.art_path && this.artUrl) return this.artUrl;
    const url = player.track?.art_url;
    return url && /^https?:/.test(url) ? url : null;
  }

  /**
   * Artwork usable as a WebGL texture: must be same-origin (our blob), since
   * cross-origin images such as Spotify's CDN can't be uploaded to WebGL.
   */
  get textureSrc(): string | null {
    if (this.forTrack?.art_path && this.artUrl) return this.artUrl;
    const url = player.track?.art_url;
    return url && url.startsWith(location.origin) ? url : null;
  }

  get backgroundSrc(): string | null {
    return this.forTrack?.background_path ? this.backgroundUrl : null;
  }

  private async loadImages(c: PaletteChanged | null): Promise<void> {
    const art = c?.art_path ?? null;
    const bg = c?.background_path ?? null;
    const [artUrl, bgUrl] = await Promise.all([
      art ? this.blobs.load(art) : null,
      bg ? this.blobs.load(bg) : null,
    ]);
    if (this.current !== c) return; // superseded by a newer track
    this.artUrl = artUrl;
    this.backgroundUrl = bgUrl;
    this.blobs.prune([art, bg]);
  }

  async start(): Promise<() => void> {
    const unlisten = await listen<PaletteChanged>("palette-changed", (e) => {
      this.current = e.payload;
      void this.loadImages(e.payload);
    });
    try {
      const snap = await invoke<PaletteChanged | null>("get_palette");
      if (snap && !this.current) {
        this.current = snap;
        void this.loadImages(snap);
      }
    } catch (err) {
      console.error("get_palette failed", err);
    }
    // Colors live in registered CSS custom properties so they transition smoothly.
    const stopEffect = $effect.root(() => {
      $effect(() => {
        const p = this.palette;
        const root = document.documentElement.style;
        for (const role of ROLES) root.setProperty(`--c-${role.replace("_", "-")}`, p[role]);
        p.ambient.forEach((c, i) => root.setProperty(`--c-amb-${i}`, c));
      });
    });
    return () => {
      unlisten();
      stopEffect();
    };
  }
}

export const palette = new PaletteStore();
