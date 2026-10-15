<script lang="ts">
  import type { TrackInfo } from "../types";
  import CoverArt from "./CoverArt.svelte";

  /** Big artwork + track info, shown when there are no lyrics to display. */
  let { track, message, action }: {
    track: TrackInfo;
    message: string;
    action?: { label: string; run: () => void };
  } = $props();
</script>

<div class="hero">
  <div class="art">
    <CoverArt />
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
