<script lang="ts">
  import { player } from "../stores/player.svelte";
  import { t } from "../i18n/index.svelte";

  const playing = $derived(player.status === "playing");
</script>

<div class="controls">
  <button onclick={player.previous} aria-label={t("controls.previous")} title={t("controls.previous")}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 6h2v12H7zM10.5 12 19 6v12z" /></svg>
  </button>
  <button
    class="main"
    onclick={player.playPause}
    aria-label={playing ? t("controls.pause") : t("controls.play")}
    title={playing ? t("controls.pause") : t("controls.play")}
  >
    {#if playing}
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" /></svg>
    {:else}
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5v14l11-7z" /></svg>
    {/if}
  </button>
  <button onclick={player.next} aria-label={t("controls.next")} title={t("controls.next")}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M15 6h2v12h-2zM13.5 12 5 18V6z" /></svg>
  </button>
</div>

<style>
  .controls {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 1.25rem;
  }
  button {
    display: grid;
    place-items: center;
    width: 2.75rem;
    height: 2.75rem;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--text);
    opacity: 0.8;
    cursor: pointer;
    transition: opacity 150ms, transform 150ms, background-color 150ms;
  }
  button:hover {
    opacity: 1;
    background: var(--hover);
  }
  button:active {
    transform: scale(0.92);
  }
  button.main {
    width: 3.5rem;
    height: 3.5rem;
    background: var(--text);
    color: var(--bg);
    opacity: 1;
  }
  button.main:hover {
    transform: scale(1.05);
  }
  svg {
    width: 1.5rem;
    height: 1.5rem;
    fill: currentColor;
  }
  button:focus-visible {
    outline: 2px solid var(--text);
    outline-offset: 3px;
  }
</style>
