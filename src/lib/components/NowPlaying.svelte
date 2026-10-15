<script lang="ts">
  import type { TrackInfo } from "../types";
  import { player } from "../stores/player.svelte";
  import { palette } from "../stores/palette.svelte";
  import { t } from "../i18n/index.svelte";
  import Controls from "./Controls.svelte";
  import ProgressBar from "./ProgressBar.svelte";

  let { track }: { track: TrackInfo } = $props();

  const artSrc = $derived(palette.artSrc);
  let artFailed = $state(false);
  $effect(() => {
    void artSrc;
    artFailed = false;
  });
</script>

<article class="now-playing">
  <div class="art">
    {#if artSrc && !artFailed}
      <img
        src={artSrc}
        alt=""
        onerror={() => {
          console.warn("artwork failed to load", artSrc);
          artFailed = true;
        }}
      />
    {:else}
      <div class="placeholder" aria-label={t("track.noArt")}>
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3z" />
        </svg>
      </div>
    {/if}
  </div>

  <div class="meta">
    <h1 title={track.title}>{track.title}</h1>
    <p class="artist" title={track.artist}>{track.artist}</p>
    {#if track.album}
      <p class="album" title={track.album}>{track.album}</p>
    {/if}
  </div>

  <ProgressBar />
  <Controls />

  <p class="hint" aria-live="polite">
    {#if player.status !== "playing"}
      {t("status.paused")}
    {:else if player.source === "estimated"}
      {t("status.estimated")}
    {/if}
  </p>
</article>

<style>
  .now-playing {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1.25rem;
    width: min(28rem, 100%);
  }
  .art {
    width: min(20rem, 60vh, 100%);
    aspect-ratio: 1;
    border-radius: 12px;
    overflow: hidden;
    box-shadow: 0 24px 60px rgb(0 0 0 / 0.45);
    background: var(--surface);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .placeholder {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    color: var(--text-dim);
  }
  .placeholder svg {
    width: 30%;
    fill: currentColor;
  }
  .meta {
    text-align: center;
    width: 100%;
    min-width: 0;
  }
  h1,
  p {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  h1 {
    font-size: 1.6rem;
    font-weight: 800;
    letter-spacing: -0.01em;
  }
  .artist {
    margin-top: 0.3rem;
    font-size: 1.05rem;
    color: var(--text);
    opacity: 0.85;
  }
  .album {
    margin-top: 0.15rem;
    font-size: 0.9rem;
    color: var(--text-dim);
  }
  .hint {
    min-height: 1.2em;
    font-size: 0.85rem;
    color: var(--text-dim);
    white-space: normal;
    text-align: center;
  }
</style>
