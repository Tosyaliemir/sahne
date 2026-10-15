<script lang="ts">
  import type { TrackInfo } from "../types";
  import { palette } from "../stores/palette.svelte";
  import { t } from "../i18n/index.svelte";

  /** Big artwork + track info, shown when there are no lyrics to display. */
  let { track, message, action }: {
    track: TrackInfo;
    message: string;
    action?: { label: string; run: () => void };
  } = $props();

  let failed = $state(false);
  $effect(() => {
    void palette.artSrc;
    failed = false;
  });
</script>

<div class="hero">
  <div class="art">
    {#if palette.artSrc && !failed}
      <img src={palette.artSrc} alt="" onerror={() => (failed = true)} />
    {:else}
      <div class="placeholder" aria-label={t("track.noArt")}>♪</div>
    {/if}
  </div>
  <div class="info">
    <h1>{track.title}</h1>
    <p class="artist">{track.artist}</p>
    <p class="message">
      {message}
      {#if action}
        <button onclick={action.run}>{action.label}</button>
      {/if}
    </p>
  </div>
</div>

<style>
  .hero {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: clamp(2rem, 5vw, 5rem);
    padding: 0 8vw 10vh;
  }
  .art {
    flex: none;
    width: min(42vh, 36vw);
    aspect-ratio: 1;
    border-radius: 14px;
    overflow: hidden;
    box-shadow: 0 30px 80px rgb(0 0 0 / 0.5);
    background: var(--surface);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .placeholder {
    display: grid;
    place-items: center;
    height: 100%;
    font-size: 6rem;
    color: var(--text-dim);
  }
  .info {
    min-width: 0;
    max-width: 40vw;
  }
  h1 {
    margin: 0;
    font-size: clamp(2rem, 4.5vw, 4.5rem);
    font-weight: 800;
    line-height: 1.05;
    letter-spacing: -0.02em;
  }
  .artist {
    margin: 0.6rem 0 0;
    font-size: clamp(1.1rem, 2vw, 1.8rem);
    font-weight: 600;
    opacity: 0.85;
  }
  .message {
    margin: 1.8rem 0 0;
    color: var(--text-dim);
    font-size: 1.05rem;
  }
  button {
    display: inline-block;
    margin-left: 0.6rem;
    white-space: nowrap;
    font: inherit;
    font-weight: 600;
    color: var(--text);
    background: var(--hover);
    border: 0;
    border-radius: 999px;
    padding: 0.35rem 1rem;
    cursor: pointer;
  }
  button:hover {
    background: var(--rail);
  }
  @media (max-aspect-ratio: 1/1) {
    .hero {
      flex-direction: column;
      text-align: center;
    }
    .info {
      max-width: 80vw;
    }
  }
</style>
