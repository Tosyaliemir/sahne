<script lang="ts">
  import { onMount } from "svelte";
  import { fly, fade } from "svelte/transition";
  import { invoke } from "@tauri-apps/api/core";
  import { settings } from "../stores/settings.svelte";
  import { player } from "../stores/player.svelte";
  import { lyrics } from "../stores/lyrics.svelte";
  import { STYLES, ui } from "../stores/ui.svelte";
  import { t } from "../i18n/index.svelte";
  import AboutSection from "./AboutSection.svelte";
  import { focusTrap } from "../focusTrap";
  import type { Settings } from "../types";

  let { onClose }: { onClose: () => void } = $props();
  let closeButton: HTMLButtonElement | undefined = $state();

  const s = $derived(settings.current);
  let players = $state<string[]>([]);

  /** Bytes used by the lyrics and artwork caches; null until known. */
  let cacheBytes = $state<number | null>(null);
  let clearing = $state(false);

  onMount(() => {
    invoke<string[]>("list_players")
      .then((p) => (players = p))
      .catch((err) => console.warn("list_players failed", err));
    invoke<number>("cache_size")
      .then((n) => (cacheBytes = n))
      .catch((err) => console.warn("cache_size failed", err));
  });

  async function clearCache() {
    clearing = true;
    try {
      await invoke<number>("clear_cache");
      cacheBytes = await invoke<number>("cache_size");
    } catch (err) {
      console.warn("clear_cache failed", err);
    } finally {
      clearing = false;
    }
  }

  const megabytes = (n: number) => (n / (1024 * 1024)).toFixed(n < 10 * 1024 * 1024 ? 1 : 0);

  function set<K extends keyof Settings>(key: K, value: Settings[K]) {
    settings.update({ [key]: value } as Partial<Settings>);
  }

  const signed = (n: number) => (n > 0 ? `+${n}` : String(n));
</script>

<div
  class="scrim"
  transition:fade={{ duration: ui.reducedMotion ? 0 : 200 }}
  onclick={onClose}
  role="presentation"
></div>
<div
  class="panel"
  role="dialog"
  aria-modal="true"
  aria-labelledby="settings-title"
  transition:fly={{ x: 420, duration: ui.reducedMotion ? 0 : 320, opacity: 1 }}
  {@attach focusTrap(() => closeButton)}
>
  <header>
    <h2 id="settings-title">{t("settings.title")}</h2>
    <button class="close" bind:this={closeButton} onclick={onClose} aria-label={t("settings.close")}>✕</button
    >
  </header>

  <section>
    <h3>{t("settings.appearance")}</h3>
    <label class="row">
      <span>{t("settings.language")}</span>
      <select
        value={s.language}
        onchange={(e) => set("language", e.currentTarget.value as Settings["language"])}
      >
        <option value="system">{t("settings.languageSystem")}</option>
        <option value="tr">Türkçe</option>
        <option value="en">English</option>
      </select>
    </label>
    <label class="row">
      <span>{t("settings.style")}</span>
      <select
        value={s.lyrics_style}
        onchange={(e) => set("lyrics_style", e.currentTarget.value as Settings["lyrics_style"])}
      >
        {#each STYLES as style (style)}
          <option value={style}>{t(`style.${style}` as const)}</option>
        {/each}
      </select>
    </label>
    <p class="hint">{t(`styleHint.${s.lyrics_style}` as const)}</p>
    <label class="row">
      <span>{t("settings.font")}</span>
      <select value={s.font} onchange={(e) => set("font", e.currentTarget.value as Settings["font"])}>
        <option value="embedded">Manrope</option>
        <option value="system">{t("settings.fontSystem")}</option>
      </select>
    </label>
    <label class="row">
      <span>{t("settings.fontScale")} <em>{Math.round(s.font_scale * 100)}%</em></span>
      <input
        type="range"
        min="0.6"
        max="1.6"
        step="0.05"
        value={s.font_scale}
        oninput={(e) => set("font_scale", +e.currentTarget.value)}
      />
    </label>
    <label class="row">
      <span>{t("settings.backdrop")}</span>
      <select
        value={s.backdrop_style}
        onchange={(e) => set("backdrop_style", e.currentTarget.value as Settings["backdrop_style"])}
      >
        {#each ["fluid", "blurred", "cover", "color"] as const as b (b)}
          <option value={b}>{t(`backdrop.${b}` as const)}</option>
        {/each}
      </select>
    </label>
    <label class="row">
      <span>{t("settings.performance")}</span>
      <select
        value={s.performance_mode}
        onchange={(e) => set("performance_mode", e.currentTarget.value as Settings["performance_mode"])}
      >
        {#each ["auto", "quality", "battery"] as const as m (m)}
          <option value={m}>{t(`performance.${m}` as const)}</option>
        {/each}
      </select>
    </label>
    <label class="row check">
      <span>{t("settings.edgeGlow")}</span>
      <input
        type="checkbox"
        checked={s.edge_glow}
        onchange={(e) => set("edge_glow", e.currentTarget.checked)}
      />
    </label>
    <label class="row">
      <span>{t("settings.coverEffect")}</span>
      <select
        value={s.cover_effect}
        onchange={(e) => set("cover_effect", e.currentTarget.value as Settings["cover_effect"])}
      >
        {#each ["none", "metal", "electric"] as const as c (c)}
          <option value={c}>{t(`coverEffect.${c}` as const)}</option>
        {/each}
      </select>
    </label>
    {#if s.backdrop_style === "blurred"}
      <label class="row">
        <span>{t("settings.blur")} <em>{s.backdrop_blur}px</em></span>
        <input
          type="range"
          min="0"
          max="60"
          step="5"
          value={s.backdrop_blur}
          oninput={(e) => set("backdrop_blur", +e.currentTarget.value)}
        />
      </label>
    {/if}
    <label class="row">
      <span>{t("settings.dim")} <em>{Math.round(s.backdrop_dim * 100)}%</em></span>
      <input
        type="range"
        min="0"
        max="0.85"
        step="0.05"
        value={s.backdrop_dim}
        oninput={(e) => set("backdrop_dim", +e.currentTarget.value)}
      />
    </label>
  </section>

  <section>
    <h3>{t("settings.lyrics")}</h3>
    <label class="row">
      <span>{t("settings.globalOffset")} <em>{signed(s.global_offset_ms)} ms</em></span>
      <input
        type="range"
        min="-2000"
        max="2000"
        step="50"
        value={s.global_offset_ms}
        oninput={(e) => set("global_offset_ms", +e.currentTarget.value)}
      />
    </label>
    {#if lyrics.forTrack}
      <div class="row">
        <span>{t("settings.trackOffset")} <em>{signed(lyrics.forTrack.offset_ms)} ms</em></span>
        <button
          class="small"
          disabled={lyrics.forTrack.offset_ms === 0}
          onclick={() => lyrics.nudge(-(lyrics.forTrack?.offset_ms ?? 0))}>{t("settings.reset")}</button
        >
      </div>
    {/if}
    <p class="hint">{t("settings.offsetHint")}</p>
    <label class="row check">
      <span>{t("settings.announceLyrics")}</span>
      <input
        type="checkbox"
        checked={s.announce_lyrics}
        onchange={(e) => set("announce_lyrics", e.currentTarget.checked)}
      />
    </label>
    <p class="hint">{t("settings.announceHint")}</p>
  </section>

  <section>
    <h3>{t("settings.player")}</h3>
    <label class="row">
      <span>{t("settings.preferredPlayer")}</span>
      <select
        value={s.preferred_player ?? ""}
        onchange={(e) => set("preferred_player", e.currentTarget.value || null)}
      >
        <option value="">{t("settings.playerAuto")}</option>
        {#each [...new Set([...players, ...(s.preferred_player ? [s.preferred_player] : [])])] as p (p)}
          <option value={p}>{p}</option>
        {/each}
      </select>
    </label>
    {#if player.playerName}
      <p class="hint">{t("settings.following", { player: player.playerName })}</p>
    {/if}
    {#if player.source === "estimated"}
      <p class="hint warn">{t("status.estimated")}</p>
    {/if}
  </section>

  <section>
    <h3>{t("settings.screensaver")}</h3>
    <label class="row check">
      <span>{t("settings.screensaverEnabled")}</span>
      <input
        type="checkbox"
        checked={s.screensaver_enabled}
        onchange={(e) => set("screensaver_enabled", e.currentTarget.checked)}
      />
    </label>
    {#if s.screensaver_enabled}
      <label class="row">
        <span
          >{t("settings.screensaverAfter")}
          <em>{t("settings.minutes", { n: String(s.screensaver_minutes) })}</em></span
        >
        <input
          type="range"
          min="1"
          max="30"
          step="1"
          value={s.screensaver_minutes}
          oninput={(e) => set("screensaver_minutes", +e.currentTarget.value)}
        />
      </label>
      <label class="row check">
        <span>{t("settings.keepAwake")}</span>
        <input
          type="checkbox"
          checked={s.screensaver_keep_awake}
          onchange={(e) => set("screensaver_keep_awake", e.currentTarget.checked)}
        />
      </label>
      <p class="hint">{t("settings.screensaverHint")}</p>
    {/if}
  </section>

  <section>
    <h3>{t("settings.window")}</h3>
    <label class="row">
      <span>{t("settings.controlsHide")}</span>
      <select
        value={String(s.controls_hide_secs)}
        onchange={(e) => set("controls_hide_secs", +e.currentTarget.value)}
      >
        {#each [1, 2, 3, 5, 10] as n (n)}
          <option value={String(n)}>{t("settings.seconds", { n: String(n) })}</option>
        {/each}
        <option value="0">{t("settings.neverHide")}</option>
      </select>
    </label>
    <label class="row check">
      <span>{t("settings.startFullscreen")}</span>
      <input
        type="checkbox"
        checked={s.start_fullscreen}
        onchange={(e) => set("start_fullscreen", e.currentTarget.checked)}
      />
    </label>
    {#if s.mini_position}
      <div class="row">
        <span>{t("settings.miniPosition")}</span>
        <button class="small" onclick={() => set("mini_position", null)}>{t("settings.reset")}</button>
      </div>
    {/if}
    <div class="row">
      <span>
        {t("settings.cache")}
        {#if cacheBytes !== null}<em>{t("settings.cacheSize", { mb: megabytes(cacheBytes) })}</em>{/if}
      </span>
      <button class="small" disabled={clearing || cacheBytes === 0} onclick={clearCache}>
        {t("settings.clearCache")}
      </button>
    </div>
  </section>

  <AboutSection />
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.35);
    z-index: 20;
  }
  .panel {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(26rem, 92vw);
    overflow-y: auto;
    padding: 1.25rem 1.5rem 2rem;
    box-sizing: border-box;
    background: color-mix(in srgb, var(--c-dark-bg) 82%, transparent);
    backdrop-filter: blur(30px) saturate(1.3);
    box-shadow: -20px 0 60px rgb(0 0 0 / 0.35);
    z-index: 21;
    user-select: none;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2 {
    margin: 0;
    font-size: 1.4rem;
    font-weight: 800;
  }
  h3 {
    margin: 1.6rem 0 0.6rem;
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.45rem 0;
  }
  .row > span {
    font-size: 0.95rem;
  }
  em {
    font-style: normal;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
    margin-left: 0.35rem;
  }
  select,
  button.small,
  .close {
    font: inherit;
    font-size: 0.9rem;
    color: var(--text);
    background: var(--hover);
    border: 0;
    border-radius: 8px;
    padding: 0.35rem 0.6rem;
    cursor: pointer;
  }
  select option {
    background: var(--c-dark-bg);
  }
  button.small:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .close {
    width: 2rem;
    height: 2rem;
    padding: 0;
    border-radius: 50%;
  }
  input[type="range"] {
    width: 9rem;
    accent-color: var(--accent);
  }
  input[type="checkbox"] {
    width: 1.1rem;
    height: 1.1rem;
    accent-color: var(--accent);
  }
  .hint {
    margin: 0.3rem 0 0;
    font-size: 0.82rem;
    line-height: 1.45;
    color: var(--text-dim);
  }
  .hint.warn {
    color: color-mix(in srgb, var(--accent) 80%, var(--text));
  }
  /* No per-frame re-blur while the backdrop moves (see ControlBar). */
  :global(html:not(.idle)) .panel {
    backdrop-filter: none;
    background: color-mix(in srgb, var(--c-dark-bg) 93%, transparent);
  }
</style>
