<script lang="ts">
  import { onMount } from "svelte";
  import { fly, fade } from "svelte/transition";
  import { invoke } from "@tauri-apps/api/core";
  import { settings } from "../stores/settings.svelte";
  import { player } from "../stores/player.svelte";
  import { lyrics } from "../stores/lyrics.svelte";
  import { STYLES, ui } from "../stores/ui.svelte";
  import { t } from "../i18n/index.svelte";
  import type { Settings } from "../types";

  let { onClose }: { onClose: () => void } = $props();

  const s = $derived(settings.current);
  let players = $state<string[]>([]);

  onMount(() => {
    invoke<string[]>("list_players")
      .then((p) => (players = p))
      .catch((err) => console.warn("list_players failed", err));
  });

  function set<K extends keyof Settings>(key: K, value: Settings[K]) {
    settings.update({ [key]: value } as Partial<Settings>);
  }

  const signed = (n: number) => (n > 0 ? `+${n}` : String(n));
</script>

<div class="scrim" transition:fade={{ duration: ui.reducedMotion ? 0 : 200 }} onclick={onClose} role="presentation"></div>
<aside
  class="panel"
  transition:fly={{ x: 420, duration: ui.reducedMotion ? 0 : 320, opacity: 1 }}
  aria-label={t("settings.title")}
>
  <header>
    <h2>{t("settings.title")}</h2>
    <button class="close" onclick={onClose} aria-label={t("settings.close")}>✕</button>
  </header>

  <section>
    <h3>{t("settings.appearance")}</h3>
    <label class="row">
      <span>{t("settings.language")}</span>
      <select value={s.language} onchange={(e) => set("language", e.currentTarget.value as Settings["language"])}>
        <option value="tr">Türkçe</option>
        <option value="en">English</option>
      </select>
    </label>
    <div class="row">
      <span>{t("settings.style")}</span>
      <div class="segmented" role="radiogroup">
        {#each STYLES as style (style)}
          <button
            role="radio"
            aria-checked={s.lyrics_style === style}
            class:on={s.lyrics_style === style}
            onclick={() => set("lyrics_style", style)}>{t(`style.${style}` as const)}</button
          >
        {/each}
      </div>
    </div>
    <label class="row">
      <span>{t("settings.font")}</span>
      <select value={s.font} onchange={(e) => set("font", e.currentTarget.value as Settings["font"])}>
        <option value="embedded">Manrope</option>
        <option value="system">{t("settings.fontSystem")}</option>
      </select>
    </label>
    <label class="row">
      <span>{t("settings.fontScale")} <em>{Math.round(s.font_scale * 100)}%</em></span>
      <input type="range" min="0.6" max="1.6" step="0.05" value={s.font_scale}
        oninput={(e) => set("font_scale", +e.currentTarget.value)} />
    </label>
    <label class="row">
      <span>{t("settings.blur")} <em>{s.backdrop_blur}px</em></span>
      <input type="range" min="0" max="60" step="5" value={s.backdrop_blur}
        oninput={(e) => set("backdrop_blur", +e.currentTarget.value)} />
    </label>
    <label class="row">
      <span>{t("settings.dim")} <em>{Math.round(s.backdrop_dim * 100)}%</em></span>
      <input type="range" min="0" max="0.85" step="0.05" value={s.backdrop_dim}
        oninput={(e) => set("backdrop_dim", +e.currentTarget.value)} />
    </label>
  </section>

  <section>
    <h3>{t("settings.lyrics")}</h3>
    <label class="row">
      <span>{t("settings.globalOffset")} <em>{signed(s.global_offset_ms)} ms</em></span>
      <input type="range" min="-2000" max="2000" step="50" value={s.global_offset_ms}
        oninput={(e) => set("global_offset_ms", +e.currentTarget.value)} />
    </label>
    {#if lyrics.forTrack}
      <div class="row">
        <span>{t("settings.trackOffset")} <em>{signed(lyrics.forTrack.offset_ms)} ms</em></span>
        <button class="small" disabled={lyrics.forTrack.offset_ms === 0}
          onclick={() => lyrics.nudge(-(lyrics.forTrack?.offset_ms ?? 0))}>{t("settings.reset")}</button>
      </div>
    {/if}
    <p class="hint">{t("settings.offsetHint")}</p>
  </section>

  <section>
    <h3>{t("settings.player")}</h3>
    <label class="row">
      <span>{t("settings.preferredPlayer")}</span>
      <select value={s.preferred_player ?? ""}
        onchange={(e) => set("preferred_player", e.currentTarget.value || null)}>
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
    <h3>{t("settings.window")}</h3>
    <label class="row check">
      <span>{t("settings.startFullscreen")}</span>
      <input type="checkbox" checked={s.start_fullscreen}
        onchange={(e) => set("start_fullscreen", e.currentTarget.checked)} />
    </label>
    {#if s.mini_position}
      <div class="row">
        <span>{t("settings.miniPosition")}</span>
        <button class="small" onclick={() => set("mini_position", null)}>{t("settings.reset")}</button>
      </div>
    {/if}
  </section>

</aside>

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
  .segmented {
    display: flex;
    background: var(--hover);
    border-radius: 8px;
    padding: 2px;
  }
  .segmented button {
    font: inherit;
    font-size: 0.85rem;
    color: var(--text-dim);
    background: transparent;
    border: 0;
    border-radius: 6px;
    padding: 0.3rem 0.6rem;
    cursor: pointer;
  }
  .segmented button.on {
    color: var(--c-dark-bg);
    background: var(--text);
    font-weight: 700;
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
</style>
