<script lang="ts">
  import { onPlaybackFrame } from "../playbackFrames";
  import { onMount } from "svelte";
  import type { LyricsLoaded } from "../types";
  import { player } from "../stores/player.svelte";
  import { lyrics } from "../stores/lyrics.svelte";
  import { activeLineIndex, blendPlan, lyricTime } from "../timing";
  import LyricsView from "./LyricsView.svelte";
  import KineticView from "./KineticView.svelte";

  /**
   * Classic flow that hands over to the kinetic style for fast passages
   * (see blendPlan) and back again. Going kinetic is quick, to catch the
   * moment; coming back waits a beat and eases in slowly, so the switch back
   * to the full line stack doesn't feel like a jolt.
   */
  let { data }: { data: LyricsLoaded } = $props();

  /** Stay kinetic this long after a fast passage ends. */
  const RETURN_DELAY_MS = 800;

  const plan = $derived(blendPlan(data.lines));
  let kinetic = $state(false);

  onMount(() => {
    let calmSince = -1;
    return onPlaybackFrame((now) => {
      const i = activeLineIndex(data.lines, lyricTime(player.clock.position(now), lyrics.totalOffsetMs));
      const fast = i >= 0 && plan[i] === true;
      if (fast) {
        calmSince = -1;
        if (!kinetic) kinetic = true;
      } else if (kinetic) {
        if (calmSince < 0) calmSince = now;
        else if (now - calmSince >= RETURN_DELAY_MS) kinetic = false;
      }
    });
  });
</script>

<div class="layer classic" class:shown={!kinetic}><LyricsView {data} style="classic" /></div>
<div class="layer kinetic" class:shown={kinetic}><KineticView {data} /></div>

<style>
  .layer {
    position: absolute;
    inset: 0;
    opacity: 0;
    pointer-events: none;
    will-change: opacity, transform;
  }
  .layer.shown {
    opacity: 1;
    transform: none;
    pointer-events: auto;
  }

  /* Into kinetic: fast. Classic shrinks back a touch as the big words land. */
  .classic {
    transform: scale(0.94);
    transition:
      opacity 380ms ease,
      transform 420ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .kinetic.shown {
    transition:
      opacity 320ms ease,
      transform 380ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  /* Back to classic: slow and soft. The kinetic word drifts up and away while
     the line stack settles in from slightly smaller. */
  .kinetic {
    transform: scale(1.06);
    transition:
      opacity 900ms ease,
      transform 1100ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .classic.shown {
    transition:
      opacity 1100ms ease 150ms,
      transform 1300ms cubic-bezier(0.16, 1, 0.3, 1) 150ms;
  }
</style>
