<script lang="ts">
  import { onMount } from "svelte";
  import { player } from "../stores/player.svelte";

  let { text }: { text: string } = $props();

  let viewport: HTMLElement | undefined = $state();
  let content: HTMLElement | undefined = $state();

  // Unsynced lyrics scroll slowly in proportion to how far the song has played.
  onMount(() => {
    let raf = requestAnimationFrame(function loop(now) {
      const length = player.track?.length_ms ?? 0;
      if (viewport && content && length > 0) {
        const progress = Math.min(1, player.clock.position(now) / length);
        const travel = Math.max(0, content.offsetHeight - viewport.offsetHeight * 0.5);
        content.style.transform = `translateY(${-progress * travel}px)`;
      }
      raf = requestAnimationFrame(loop);
    });
    return () => cancelAnimationFrame(raf);
  });
</script>

<div class="viewport" bind:this={viewport}>
  <div class="content" bind:this={content}>{text}</div>
</div>

<style>
  .viewport {
    position: absolute;
    inset: 0;
    overflow: hidden;
    mask-image: linear-gradient(transparent, #000 20%, #000 80%, transparent);
  }
  .content {
    padding: 35vh 10vw;
    white-space: pre-wrap;
    text-align: center;
    font-size: calc(clamp(1.4rem, 2.6vw, 2.6rem) * var(--lyrics-scale, 1));
    font-weight: 700;
    line-height: 1.5;
    will-change: transform;
  }
</style>
