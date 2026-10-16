<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { invoke } from "@tauri-apps/api/core";
  import { player } from "./lib/stores/player.svelte";
  import { palette } from "./lib/stores/palette.svelte";
  import { settings } from "./lib/stores/settings.svelte";
  import { ui } from "./lib/stores/ui.svelte";
  import { t } from "./lib/i18n/index.svelte";
  import { formatTime } from "./lib/format";
  import { Lifecycle } from "./lib/lifecycle";
  import VolumeControl from "./lib/components/VolumeControl.svelte";
  import Background from "./lib/components/Background.svelte";
  import FlowBackground from "./lib/components/FlowBackground.svelte";

  let progressEl: HTMLElement | undefined = $state();
  /** WebGL missing or failing: fall back to the CSS backdrop. */
  let flowUnsupported = $state(false);
  const playing = $derived(player.status === "playing");

  onMount(() => {
    const life = new Lifecycle();
    life.add(settings.start());
    life.add(player.start());
    life.add(palette.start());

    let raf = requestAnimationFrame(function loop(now) {
      const pos = player.positionNow(now);
      const length = player.track?.length_ms ?? 0;
      if (progressEl && length > 0) progressEl.style.transform = `scaleX(${Math.min(1, pos / length)})`;
      raf = requestAnimationFrame(loop);
    });

    return () => {
      cancelAnimationFrame(raf);
      life.dispose();
    };
  });

  function expand() {
    invoke("set_mini_mode", { enabled: false }).catch((err) => console.warn("set_mini_mode failed", err));
  }

  function onKey(e: KeyboardEvent) {
    if (e.target instanceof HTMLElement && e.target.closest("[role=slider]")) return;
    if (e.key === " ") {
      e.preventDefault();
      player.playPause();
    } else if (e.key === "m" || e.key === "M" || e.key === "Escape") {
      expand();
    } else if (e.key === "ArrowLeft") {
      player.previous();
    } else if (e.key === "ArrowRight") {
      player.next();
    } else if ((e.key === "ArrowUp" || e.key === "ArrowDown") && player.volume !== null) {
      e.preventDefault();
      player.setVolume(player.volume + (e.key === "ArrowUp" ? 0.05 : -0.05));
    }
  }

  let artFailed = $state(false);
  $effect(() => {
    void palette.artSrc;
    artFailed = false;
  });
  const fadeMs = $derived(ui.reducedMotion ? 0 : 600);
</script>

<svelte:window onkeydown={onKey} />

<div class="mini" class:playing data-tauri-drag-region>
  <!-- Same backdrop as the stage, per settings; .mini contains and clips it. -->
  {#if settings.current.backdrop_style === "akiskan" && !flowUnsupported}
    <FlowBackground onUnsupported={() => (flowUnsupported = true)} />
  {:else}
    <Background mode={settings.current.backdrop_style === "akiskan" ? "bulanik" : settings.current.backdrop_style} />
  {/if}
  <div class="shade" aria-hidden="true"></div>

  <div class="cover" data-tauri-drag-region>
    {#key palette.artSrc}
      {#if palette.artSrc && !artFailed}
        <img src={palette.artSrc} alt="" onerror={() => (artFailed = true)} transition:fade={{ duration: fadeMs }} />
      {:else}
        <div class="placeholder">♪</div>
      {/if}
    {/key}
  </div>

  <div class="body" data-tauri-drag-region>
    {#if player.track}
      <div class="meta" data-tauri-drag-region>
        <span class="eq" aria-hidden="true"><i></i><i></i><i></i></span>
        {#key player.track.title}
          <span class="title" data-tauri-drag-region in:fade={{ duration: fadeMs }}>{player.track.title}</span>
        {/key}
        <span class="artist" data-tauri-drag-region>{player.track.artist}</span>
        {#if player.track.album}
          <span class="album" data-tauri-drag-region>{player.track.album}</span>
        {/if}
      </div>

      <div class="bottom">
        <span class="time">{formatTime(player.positionMs)}</span>
        <div class="rail"><div class="fill" bind:this={progressEl}></div></div>
        <div class="buttons">
          <button onclick={player.previous} aria-label={t("controls.previous")} title={t("controls.previous")}>
            <svg viewBox="0 0 24 24"><path d="M7 6h2v12H7zM10.5 12 19 6v12z" /></svg>
          </button>
          <button class="main" onclick={player.playPause} aria-label={playing ? t("controls.pause") : t("controls.play")}>
            {#if playing}
              <svg viewBox="0 0 24 24"><path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" /></svg>
            {:else}
              <svg viewBox="0 0 24 24"><path d="M8 5v14l11-7z" /></svg>
            {/if}
          </button>
          <button onclick={player.next} aria-label={t("controls.next")} title={t("controls.next")}>
            <svg viewBox="0 0 24 24"><path d="M15 6h2v12h-2zM13.5 12 5 18V6z" /></svg>
          </button>
        </div>
        <div class="vol"><VolumeControl /></div>
      </div>
    {:else}
      <div class="meta"><span class="title">{t("status.noPlayer.title")}</span></div>
    {/if}
  </div>

  <button class="expand" onclick={expand} title={t("mini.expand")} aria-label={t("mini.expand")}>
    <svg viewBox="0 0 24 24"><path d="M14 3h7v7h-2V6.4l-5.3 5.3-1.4-1.4L17.6 5H14zM3 14h2v3.6l5.3-5.3 1.4 1.4L6.4 19H10v2H3z" /></svg>
  </button>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
  }
  .mini {
    --fill: var(--text);
    --unfilled: color-mix(in srgb, var(--text) 38%, transparent);
    position: fixed;
    inset: 6px;
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: 1rem;
    padding: 0.9rem 1rem 0.9rem 0.9rem;
    border-radius: 20px;
    overflow: hidden;
    background: var(--c-dark-bg);
    box-shadow:
      inset 0 0 0 1px color-mix(in srgb, var(--text) 12%, transparent),
      0 4px 14px rgb(0 0 0 / 0.35);
    cursor: grab;
    /* Contains the fixed-position backdrop so the rounded corners clip it. */
    transform: translateZ(0);
    isolation: isolate;
  }

  /* Keeps text readable over bright covers without hiding the backdrop. */
  .shade {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: linear-gradient(90deg, color-mix(in srgb, var(--c-dark-bg) 15%, transparent), color-mix(in srgb, var(--c-dark-bg) 55%, transparent));
  }

  .cover {
    position: relative;
    z-index: 1;
    width: 7.6rem;
    height: 7.6rem;
    border-radius: 12px;
    overflow: hidden;
    box-shadow: 0 10px 28px rgb(0 0 0 / 0.45);
    transition: transform 600ms cubic-bezier(0.22, 1, 0.36, 1);
    transform: scale(0.94);
  }
  .playing .cover {
    transform: scale(1);
    animation: breathe 5s ease-in-out infinite;
  }
  .cover img,
  .placeholder {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .placeholder {
    display: grid;
    place-items: center;
    background: var(--surface);
    color: var(--text-dim);
    font-size: 2.4rem;
  }

  .body {
    position: relative;
    z-index: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    height: 100%;
    justify-content: space-between;
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
    padding-right: 1.6rem;
  }
  .title,
  .artist,
  .album {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title {
    margin-top: 0.35rem;
    font-size: 1.3rem;
    font-weight: 800;
    letter-spacing: -0.01em;
    line-height: 1.15;
  }
  .artist {
    font-size: 0.92rem;
    font-weight: 600;
    opacity: 0.85;
  }
  .album {
    font-size: 0.78rem;
    color: var(--text-dim);
  }

  /* Tiny equalizer: bars move while playing, rest when paused. */
  .eq {
    display: inline-flex;
    align-items: flex-end;
    gap: 2px;
    height: 0.75rem;
    flex: none;
    align-self: flex-start;
  }
  .eq i {
    width: 3px;
    height: 100%;
    border-radius: 1px;
    background: var(--accent);
    transform-origin: bottom;
    transform: scaleY(0.3);
  }
  .playing .eq i {
    animation: eq 0.9s ease-in-out infinite alternate;
  }
  .playing .eq i:nth-child(2) {
    animation-duration: 0.7s;
    animation-delay: -0.3s;
  }
  .playing .eq i:nth-child(3) {
    animation-duration: 1.1s;
    animation-delay: -0.6s;
  }

  .bottom {
    display: grid;
    grid-template-columns: auto 1fr auto auto;
    align-items: center;
    gap: 0.6rem;
  }
  .time {
    font-size: 0.7rem;
    font-variant-numeric: tabular-nums;
    color: var(--text-dim);
    min-width: 2.2rem;
  }
  .rail {
    height: 3px;
    border-radius: 2px;
    background: var(--rail);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--text);
    transform-origin: left center;
    transform: scaleX(0);
  }
  .buttons {
    display: flex;
    align-items: center;
    gap: 0.15rem;
  }
  .buttons button,
  .expand {
    display: grid;
    place-items: center;
    width: 1.9rem;
    height: 1.9rem;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--text);
    opacity: 0.8;
    cursor: pointer;
    transition: opacity 150ms, background-color 150ms, transform 150ms;
  }
  .buttons button:hover,
  .expand:hover {
    opacity: 1;
    background: var(--hover);
  }
  .buttons button:active {
    transform: scale(0.9);
  }
  .buttons button.main {
    width: 2.3rem;
    height: 2.3rem;
    background: var(--text);
    color: var(--c-dark-bg);
    opacity: 1;
  }
  svg {
    width: 1.05rem;
    height: 1.05rem;
    fill: currentColor;
  }
  /* The volume control is compact here: icon plus a short slider. */
  .vol :global(.track) {
    width: 4rem;
  }
  .vol :global(button) {
    width: 1.9rem;
    height: 1.9rem;
  }
  .expand {
    position: absolute;
    top: 0.45rem;
    right: 0.45rem;
    z-index: 2;
    width: 1.6rem;
    height: 1.6rem;
    opacity: 0;
  }
  .mini:hover .expand {
    opacity: 0.75;
  }
  .expand svg {
    width: 0.85rem;
    height: 0.85rem;
  }

  @keyframes breathe {
    0%, 100% { transform: scale(1); }
    50% { transform: scale(1.025); }
  }
  @keyframes eq {
    from { transform: scaleY(0.25); }
    to { transform: scaleY(1); }
  }
  @media (prefers-reduced-motion: reduce) {
    .playing .cover,
    .playing .eq i {
      animation: none;
    }
  }
</style>
