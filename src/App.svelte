<script lang="ts">
  import { onMount } from "svelte";
  import { player } from "./lib/stores/player.svelte";
  import { t } from "./lib/i18n/index.svelte";
  import NowPlaying from "./lib/components/NowPlaying.svelte";
  import StatusScreen from "./lib/components/StatusScreen.svelte";

  onMount(() => {
    let stop: (() => void) | undefined;
    player.start().then((s) => (stop = s));
    return () => stop?.();
  });

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
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<main>
  {#if !player.connected}
    <StatusScreen title={t("status.noPlayer.title")} body={t("status.noPlayer.body")} />
  {:else if !player.track}
    <StatusScreen
      title={t("status.idle.title")}
      body={t("status.idle.body", { player: player.playerName ?? "" })}
    />
  {:else}
    <NowPlaying track={player.track} />
  {/if}
</main>

<style>
  main {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 2rem;
    box-sizing: border-box;
  }
</style>
