<script lang="ts">
  import { fade } from "svelte/transition";
  import { palette } from "../stores/palette.svelte";
  import { settings } from "../stores/settings.svelte";
  import { ui } from "../stores/ui.svelte";

  /**
   * CSS backdrops:
   * - bulanik: pre-blurred artwork with five drifting color fields
   * - kapak: the sharp original cover, slowly zooming, darkened for reading
   * - renk: only the drifting color fields, no artwork
   * Everything animates with transform/opacity only.
   */
  let { mode = "bulanik" }: { mode?: "bulanik" | "kapak" | "renk" } = $props();

  // A CSS blur on an animated full-screen layer is expensive; the backend
  // already pre-blurs the artwork, so the extra blur is opt-in.
  const extraBlur = $derived(mode === "bulanik" && settings.current.backdrop_blur > 0);
  const imageSrc = $derived(
    mode === "bulanik" ? palette.backgroundSrc : mode === "kapak" ? palette.artSrc : null,
  );
  const fadeMs = $derived(ui.reducedMotion ? 0 : 800);
</script>

<div class="backdrop {mode}" aria-hidden="true">
  {#key imageSrc}
    {#if imageSrc}
      <img
        class="art"
        class:blurred={extraBlur}
        src={imageSrc}
        alt=""
        transition:fade={{ duration: fadeMs }}
        onerror={() => console.warn("backdrop failed to load", imageSrc)}
      />
    {/if}
  {/key}
  {#if mode !== "kapak"}
    <div class="mesh">
      {#each [0, 1, 2, 3, 4] as i (i)}
        <div class="blob b{i}" style:--color="var(--c-amb-{i})"></div>
      {/each}
    </div>
  {/if}
  <div class="dim"></div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    overflow: hidden;
    background: var(--c-dark-bg);
    z-index: -1;
  }
  .art {
    position: absolute;
    inset: -12%;
    width: 124%;
    height: 124%;
    object-fit: cover;
    opacity: 0.7;
    animation: drift 48s ease-in-out infinite alternate;
    will-change: transform;
  }
  .art.blurred {
    filter: blur(var(--backdrop-blur));
  }
  /* The real cover: sharp, full-bleed, with a slow Ken Burns zoom. */
  .kapak .art {
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 1;
    animation: kenburns 60s ease-in-out infinite alternate;
  }
  .mesh {
    position: absolute;
    inset: 0;
    opacity: 0.62;
  }
  .renk .mesh {
    opacity: 0.85;
  }
  .blob {
    position: absolute;
    width: 62vmax;
    height: 62vmax;
    border-radius: 50%;
    background: radial-gradient(closest-side, var(--color), transparent);
    will-change: transform;
  }
  /* Spread around the screen; different durations so the pattern rarely repeats. */
  .b0 { top: -24vmax; left: -18vmax; animation: float-a 37s ease-in-out infinite alternate; }
  .b1 { top: -20vmax; right: -22vmax; animation: float-b 43s ease-in-out infinite alternate; }
  .b2 { bottom: -28vmax; left: 8vmax; animation: float-c 53s ease-in-out infinite alternate; }
  .b3 { bottom: -26vmax; right: -14vmax; animation: float-a 59s ease-in-out infinite alternate-reverse; }
  .b4 { top: 18vh; left: 28vw; width: 46vmax; height: 46vmax; animation: float-b 67s ease-in-out infinite alternate-reverse; }
  .dim {
    position: absolute;
    inset: 0;
    background: var(--c-dark-bg);
    opacity: var(--backdrop-dim);
  }
  /* A sharp image under the lyrics needs much more help to stay readable. */
  .kapak .dim {
    opacity: calc(0.45 + var(--backdrop-dim) * 0.6);
    background: radial-gradient(ellipse at center, var(--c-dark-bg) 0%, color-mix(in srgb, var(--c-dark-bg) 70%, transparent) 70%);
  }

  @keyframes drift {
    from { transform: scale(1) translate3d(0, 0, 0) rotate(0deg); }
    to { transform: scale(1.1) translate3d(-3%, 2%, 0) rotate(3deg); }
  }
  @keyframes kenburns {
    from { transform: scale(1.02) translate3d(0, 0, 0); }
    to { transform: scale(1.14) translate3d(-2%, -1.5%, 0); }
  }
  @keyframes float-a {
    from { transform: translate3d(0, 0, 0) scale(1); }
    to { transform: translate3d(22vw, 18vh, 0) scale(1.2); }
  }
  @keyframes float-b {
    from { transform: translate3d(0, 0, 0) scale(1.1); }
    to { transform: translate3d(-24vw, 16vh, 0) scale(0.9); }
  }
  @keyframes float-c {
    from { transform: translate3d(0, 0, 0) scale(0.95); }
    to { transform: translate3d(18vw, -20vh, 0) scale(1.18); }
  }

  @media (prefers-reduced-motion: reduce) {
    .art,
    .blob {
      animation: none;
    }
  }
  /* Frozen while paused or hidden, like every ambient animation. */
  :global(html.idle) .backdrop :global(*) {
    animation-play-state: paused;
  }
</style>
