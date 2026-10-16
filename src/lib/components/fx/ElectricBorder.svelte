<script lang="ts">
  import { untrack, type Snippet } from "svelte";
  import { createElectricBorder, electricSettings, MARGIN } from "../../fx/electricBorder";
  import { settings } from "../../stores/settings.svelte";
  import { ui } from "../../stores/ui.svelte";

  /**
   * Crackling light around `children`, in `color`. Without WebGL2 it simply
   * shows the children. `active = false` fades the light out and stops the
   * render loop (e.g. while the cover is hidden), so it costs nothing.
   * With "more contrast" requested the light glows and flickers less; the
   * battery saver drops the sparks and draws at 30 fps and lower resolution.
   */
  let {
    color,
    radius = 14,
    active = true,
    paused = false,
    children,
  }: { color: string; radius?: number; active?: boolean; paused?: boolean; children: Snippet } = $props();

  let container: HTMLElement | undefined = $state();
  let canvas: HTMLCanvasElement | undefined = $state();
  /** Bumped when the GPU context is restored, to rebuild the renderer. */
  let generation = $state(0);

  const options = $derived(
    electricSettings({
      color,
      active,
      paused,
      borderRadius: radius,
      glow: ui.moreContrast ? 0.3 : 0.9,
      intensity: 0.85,
      chaos: ui.moreContrast ? 0.25 : 0.45,
      spread: ui.moreContrast ? 0.3 : 0.55,
      flicker: ui.moreContrast ? 0 : 0.5,
      strands: 3,
      ...(settings.current.performance_mode === "battery" ? { arcs: 0, maxFps: 30, resolution: 0.5 } : {}),
    }),
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
