<script lang="ts">
  import { untrack } from "svelte";
  import { createReflectivePlate, reflectiveSettings } from "../../fx/reflectivePlate";

  /**
   * The cover set into a brushed metal plate. The plate mirrors a blurred
   * copy of the cover (`textureSrc`, same-origin for WebGL), tinted with
   * `tint`, and the whole card tilts toward the mouse.
   */
  let {
    src,
    textureSrc,
    tint,
    radius = 18,
  }: { src: string; textureSrc: string | null; tint: string; radius?: number } = $props();

  let root: HTMLElement | undefined = $state();
  let body: HTMLElement | undefined = $state();
  let canvas: HTMLCanvasElement | undefined = $state();
  let media: HTMLImageElement | undefined = $state();
  let generation = $state(0);

  const options = $derived(
    reflectiveSettings({
      tint,
      radius,
      grayscale: 0.55,
      roughness: 0.32,
      blurStrength: 8,
      glare: 0.55,
      tiltStrength: 10,
    }),
  );
  let fx: ReturnType<typeof createReflectivePlate> | null = null;

  $effect(() => {
    const gen = generation;
    if (!root || !body || !canvas || !media) return;
    const els = { root, body, canvas, image: media };
    const created = untrack(() =>
      createReflectivePlate(els, () => options, { isFirst: gen === 0, onRestore: () => generation++ }),
    );
    fx = created;
    return () => {
      created.destroy();
      if (fx === created) fx = null;
    };
  });

  $effect(() => {
    void options;
    void textureSrc;
    untrack(() => fx?.wake());
  });
</script>

<div class="metal" bind:this={root} style:--radius="{radius}px">
  <div class="body" bind:this={body}>
    <canvas bind:this={canvas} aria-hidden="true"></canvas>
    <div class="inset">
      <img {src} alt="" draggable="false" />
    </div>
    <span class="rim" aria-hidden="true"></span>
  </div>
  <!-- What the plate reflects; never shown itself. -->
  <img class="source" bind:this={media} src={textureSrc ?? undefined} alt="" aria-hidden="true" />
</div>

<style>
  .metal {
    --rc-rotate-x: 0deg;
    --rc-rotate-y: 0deg;
    --rc-presence: 1;
    --rc-lift: 0px;
    position: relative;
    width: 100%;
    height: 100%;
    perspective: 900px;
  }
  .body {
    position: relative;
    width: 100%;
    height: 100%;
    border-radius: var(--radius);
    background: #1d1c22;
    box-shadow:
      0 30px 70px -24px rgb(0 0 0 / 0.75),
      0 8px 20px -10px rgb(0 0 0 / 0.5);
    transform: translateY(var(--rc-lift)) rotateX(var(--rc-rotate-x)) rotateY(var(--rc-rotate-y));
    transform-style: preserve-3d;
    opacity: var(--rc-presence);
    will-change: transform;
  }
  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
    border-radius: var(--radius);
  }
  /* The cover sits recessed in the plate, like an inlay. */
  .inset {
    position: absolute;
    inset: 7%;
    border-radius: calc(var(--radius) * 0.55);
    overflow: hidden;
    box-shadow:
      inset 0 0 0 1px rgb(255 255 255 / 0.08),
      0 2px 3px rgb(0 0 0 / 0.55),
      0 0 0 1px rgb(0 0 0 / 0.5);
  }
  .inset img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    user-select: none;
  }
  .rim {
    position: absolute;
    inset: 0;
    border-radius: var(--radius);
    pointer-events: none;
    box-shadow:
      inset 0 0 0 1px rgb(255 255 255 / 0.07),
      inset 0 1px 0 rgb(255 255 255 / 0.1);
  }
  .source {
    position: absolute;
    width: 1px;
    height: 1px;
    opacity: 0;
    pointer-events: none;
  }
</style>
