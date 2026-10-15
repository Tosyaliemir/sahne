<script lang="ts">
  import { untrack, type Snippet } from "svelte";
  import { createElectricBorder, electricSettings, MARGIN } from "../../fx/electricBorder";

  /** Crackling light around `children`, in `color`. Without WebGL2 it simply shows the children. */
  let { color, radius = 14, children }: { color: string; radius?: number; children: Snippet } = $props();

  let container: HTMLElement | undefined = $state();
  let canvas: HTMLCanvasElement | undefined = $state();
  /** Bumped when the GPU context is restored, to rebuild the renderer. */
  let generation = $state(0);

  const options = $derived(
    electricSettings({ color, borderRadius: radius, glow: 0.9, intensity: 0.85, chaos: 0.45, spread: 0.55, strands: 3 }),
  );
  let fx: ReturnType<typeof createElectricBorder> = null;

  $effect(() => {
    const gen = generation;
    if (!container || !canvas) return;
    const el = container;
    const cv = canvas;
    const created = untrack(() =>
      createElectricBorder(el, cv, () => options, { isFirst: gen === 0, onRestore: () => generation++ }),
    );
    fx = created;
    return () => {
      created?.destroy();
      if (fx === created) fx = null;
    };
  });

  $effect(() => {
    void options;
    untrack(() => fx?.refresh());
  });
</script>

<div class="electric" bind:this={container} style:border-radius="{radius}px">
  {@render children()}
  <canvas bind:this={canvas} style:top="-{MARGIN}px" style:left="-{MARGIN}px" aria-hidden="true"></canvas>
</div>

<style>
  .electric {
    position: relative;
    width: 100%;
    height: 100%;
  }
  canvas {
    position: absolute;
    pointer-events: none;
    display: block;
  }
</style>
