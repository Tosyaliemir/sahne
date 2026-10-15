<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { player } from "./lib/stores/player.svelte";
  import { lyrics } from "./lib/stores/lyrics.svelte";
  import { palette } from "./lib/stores/palette.svelte";
  import { settings } from "./lib/stores/settings.svelte";
  import { t } from "./lib/i18n/index.svelte";
  import { activeLineIndex, lyricTime } from "./lib/timing";
  import Controls from "./lib/components/Controls.svelte";

  let lineIdx = $state(-1);
  let lineEl: HTMLElement | undefined = $state();

  onMount(() => {
    const stops: (() => void)[] = [];
    settings.start().then((s) => stops.push(s));
    player.start().then((s) => stops.push(s));
    lyrics.start().then((s) => stops.push(s));
    palette.start().then((s) => stops.push(s));

    // Active line plus a line-level fill; cheap enough for a small window.
    let raf = requestAnimationFrame(function loop(now) {
      const data = lyrics.forTrack;
      if (data?.state === "synced") {
        const tl = lyricTime(player.clock.position(now), lyrics.totalOffsetMs);
        const i = activeLineIndex(data.lines, tl);
        if (i !== lineIdx) lineIdx = i;
        const line = data.lines[i];
        if (line && lineEl) {
          const p = Math.min(1, Math.max(0, (tl - line.start_ms) / Math.max(1, line.end_ms - line.start_ms)));
          lineEl.style.setProperty("--p", p.toFixed(3));
        }
      } else if (lineIdx !== -1) {
        lineIdx = -1;
      }
      raf = requestAnimationFrame(loop);
    });

    return () => {
      cancelAnimationFrame(raf);
      stops.forEach((s) => s());
    };
  });

  function expand() {
    invoke("set_mini_mode", { enabled: false }).catch((err) => console.warn("set_mini_mode failed", err));
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === " ") {
      e.preventDefault();
      player.playPause();
    } else if (e.key === "m" || e.key === "M" || e.key === "Escape") {
      expand();
    } else if (e.key === "ArrowLeft") {
      player.previous();
    } else if (e.key === "ArrowRight") {
      player.next();
    }
  }

  const data = $derived(lyrics.forTrack);
  const line = $derived(data?.state === "synced" && lineIdx >= 0 ? data.lines[lineIdx].text : "");
  let artFailed = $state(false);
  $effect(() => {
    void palette.artSrc;
    artFailed = false;
  });
</script>

<svelte:window onkeydown={onKey} />

<div class="mini" data-tauri-drag-region>
  {#if palette.artSrc && !artFailed}
    <img class="art" src={palette.artSrc} alt="" onerror={() => (artFailed = true)} data-tauri-drag-region />
  {:else}
    <div class="art placeholder" data-tauri-drag-region>♪</div>
  {/if}
  <div class="body" data-tauri-drag-region>
    {#if player.track}
      <div class="meta" data-tauri-drag-region>
        <span class="title">{player.track.title}</span>
        <span class="artist">· {player.track.artist}</span>
      </div>
      {#key lineIdx}
        <p class="line" bind:this={lineEl} data-tauri-drag-region>{line || " "}</p>
      {/key}
    {:else}
      <div class="meta"><span class="title">{t("status.noPlayer.title")}</span></div>
    {/if}
  </div>
  <div class="side">
    <Controls />
    <button class="expand" onclick={expand} title={t("mini.expand")} aria-label={t("mini.expand")}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 3h7v2H5v5H3zm11 0h7v7h-2V5h-5zM3 14h2v5h5v2H3zm16 0h2v7h-7v-2h5z" /></svg>
    </button>
  </div>
</div>

<style>
  :global(body) {
    background: var(--c-dark-bg);
  }
  .mini {
    position: fixed;
    inset: 0;
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 0.9rem;
    padding: 0.75rem 0.9rem;
    box-sizing: border-box;
    background:
      radial-gradient(circle at 15% 20%, color-mix(in srgb, var(--c-vibrant) 45%, transparent), transparent 60%),
      radial-gradient(circle at 90% 90%, color-mix(in srgb, var(--c-accent) 35%, transparent), transparent 55%),
      var(--c-dark-bg);
    cursor: grab;
  }
  .art {
    width: 6.5rem;
    height: 6.5rem;
    border-radius: 10px;
    object-fit: cover;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.4);
  }
  .placeholder {
    display: grid;
    place-items: center;
    background: var(--surface);
    color: var(--text-dim);
    font-size: 2rem;
  }
  .body {
    min-width: 0;
  }
  .meta {
    display: flex;
    gap: 0.35rem;
    font-size: 0.8rem;
    white-space: nowrap;
    overflow: hidden;
  }
  .title {
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .artist {
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .line {
    --p: 0;
    margin: 0.35rem 0 0;
    font-size: 1.15rem;
    font-weight: 800;
    line-height: 1.2;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    background-image: linear-gradient(
      90deg,
      var(--text) calc(var(--p) * 125% - 25%),
      color-mix(in srgb, var(--text) 40%, transparent) calc(var(--p) * 125%)
    );
    background-clip: text;
    -webkit-background-clip: text;
    color: transparent;
    animation: rise 320ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
  .side {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
  }
  .side :global(.controls) {
    gap: 0.2rem;
  }
  .side :global(.controls button) {
    width: 2rem;
    height: 2rem;
  }
  .side :global(.controls button.main) {
    width: 2.4rem;
    height: 2.4rem;
  }
  .side :global(.controls svg) {
    width: 1.1rem;
    height: 1.1rem;
  }
  .expand {
    border: 0;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    width: 1.8rem;
    height: 1.4rem;
  }
  .expand:hover {
    color: var(--text);
  }
  .expand svg {
    width: 0.95rem;
    height: 0.95rem;
    fill: currentColor;
  }
</style>
