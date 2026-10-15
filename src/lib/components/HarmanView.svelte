<script lang="ts">
  import { onMount } from "svelte";
  import type { LyricsLoaded } from "../types";
  import { player } from "../stores/player.svelte";
  import { lyrics } from "../stores/lyrics.svelte";
  import { activeLineIndex, harmanPlan, lyricTime } from "../timing";
  import LyricsView from "./LyricsView.svelte";
  import KineticView from "./KineticView.svelte";

  /**
   * Classic flow that hands over to the kinetic style for fast passages
   * (see harmanPlan) and back again, cross-fading between the two.
   */
  let { data }: { data: LyricsLoaded } = $props();

  const plan = $derived(harmanPlan(data.lines));
  let kinetic = $state(false);

  onMount(() => {
    let raf = requestAnimationFrame(function loop(now) {
      const i = activeLineIndex(data.lines, lyricTime(player.clock.position(now), lyrics.totalOffsetMs));
      const want = i >= 0 && plan[i] === true;
      if (want !== kinetic) kinetic = want;
      raf = requestAnimationFrame(loop);
    });
    return () => cancelAnimationFrame(raf);
  });
</script>

<div class="layer" class:shown={!kinetic}><LyricsView {data} style="klasik" /></div>
<div class="layer" class:shown={kinetic}><KineticView {data} /></div>

<style>
  .layer {
    position: absolute;
    inset: 0;
    opacity: 0;
    transform: scale(0.97);
    transition:
      opacity 420ms ease,
      transform 520ms cubic-bezier(0.22, 1, 0.36, 1);
    pointer-events: none;
  }
  .layer.shown {
    opacity: 1;
    transform: none;
    pointer-events: auto;
  }
</style>
