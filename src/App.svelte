<script lang="ts">
  import { onMount } from "svelte";
  import { player } from "./lib/stores/player.svelte";
  import { lyrics, OFFSET_STEP_MS } from "./lib/stores/lyrics.svelte";
  import { palette } from "./lib/stores/palette.svelte";
  import Background from "./lib/components/Background.svelte";
  import { t } from "./lib/i18n/index.svelte";
  import NowPlaying from "./lib/components/NowPlaying.svelte";
  import StatusScreen from "./lib/components/StatusScreen.svelte";
  import LyricsList from "./lib/components/LyricsList.svelte";

  onMount(() => {
    const stops: (() => void)[] = [];
    player.start().then((s) => stops.push(s));
    lyrics.start().then((s) => stops.push(s));
    palette.start().then((s) => stops.push(s));
    return () => stops.forEach((s) => s());
  });

  let toast = $state<string | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  function showToast(text: string) {
    toast = text;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), 1500);
  }

  function nudge(delta: number) {
    lyrics.nudge(delta);
    const offset = lyrics.forTrack?.offset_ms;
    if (offset !== undefined) showToast(t("lyrics.offset", { value: (offset > 0 ? "+" : "") + offset }));
  }

  function onKey(e: KeyboardEvent) {
    if (e.target instanceof HTMLElement && e.target.closest("input, textarea, [role=slider]")) return;
    switch (e.key) {
      case " ":
        e.preventDefault();
        player.playPause();
        break;
      case "ArrowLeft":
        player.previous();
        break;
      case "ArrowRight":
        player.next();
        break;
      case "[":
        nudge(-OFFSET_STEP_MS);
        break;
      case "]":
        nudge(OFFSET_STEP_MS);
        break;
    }
  }

  const data = $derived(lyrics.forTrack);
</script>

<svelte:window onkeydown={onKey} />

<Background />

<main>
  {#if !player.connected}
    <StatusScreen title={t("status.noPlayer.title")} body={t("status.noPlayer.body")} />
  {:else if !player.track}
    <StatusScreen
      title={t("status.idle.title")}
      body={t("status.idle.body", { player: player.playerName ?? "" })}
    />
  {:else}
    <div class="layout">
      <NowPlaying track={player.track} />
      <section class="lyrics">
        {#if data?.state === "synced"}
          <LyricsList {data} />
        {:else if data?.state === "plain" && data.plain}
          <pre class="plain">{data.plain}</pre>
        {:else}
          <p class="lyrics-status">
            {#if !data || data.state === "loading"}
              {t("lyrics.loading")}
            {:else if data.state === "instrumental"}
              {t("lyrics.instrumental")}
            {:else if data.state === "offline"}
              {t("lyrics.offline")}
            {:else if data.state === "error"}
              {t("lyrics.error")}
              <button onclick={() => lyrics.retry()}>{t("lyrics.retry")}</button>
            {:else}
              {t("lyrics.notFound")}
            {/if}
          </p>
        {/if}
      </section>
    </div>
  {/if}
  {#if toast}
    <div class="toast" role="status">{toast}</div>
  {/if}
</main>

<style>
  main {
    height: 100vh;
    display: grid;
    place-items: center;
    padding: 2rem;
    box-sizing: border-box;
  }
  .layout {
    display: grid;
    grid-template-columns: minmax(16rem, 24rem) 1fr;
    gap: 3rem;
    align-items: center;
    width: 100%;
    height: 100%;
  }
  .lyrics {
    height: 100%;
    min-width: 0;
    display: grid;
    align-items: center;
  }
  .lyrics-status {
    color: var(--text-dim);
    font-size: 1.2rem;
  }
  .lyrics-status button {
    margin-left: 0.75rem;
    font: inherit;
    color: var(--text);
    background: var(--hover);
    border: 0;
    border-radius: 999px;
    padding: 0.3rem 0.9rem;
    cursor: pointer;
  }
  .plain {
    font: inherit;
    font-size: 1.2rem;
    line-height: 1.6;
    white-space: pre-wrap;
    max-height: 100%;
    overflow-y: auto;
    margin: 0;
  }
  .toast {
    position: fixed;
    bottom: 1.5rem;
    left: 50%;
    transform: translateX(-50%);
    padding: 0.5rem 1rem;
    border-radius: 999px;
    background: var(--surface);
    color: var(--text);
    font-size: 0.9rem;
    font-variant-numeric: tabular-nums;
  }
</style>
