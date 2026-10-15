<script lang="ts">
  import { fade } from "svelte/transition";
  import { palette } from "../stores/palette.svelte";
  import { ui } from "../stores/ui.svelte";
</script>

<!-- Layers, bottom to top: palette color, blurred artwork, mesh gradient, dimmer.
     Only transform and opacity animate; the blur is pre-rendered by the backend. -->
<div class="backdrop" aria-hidden="true">
  {#key palette.backgroundSrc}
    {#if palette.backgroundSrc}
      <img
        class="art"
        src={palette.backgroundSrc}
        alt=""
        transition:fade={{ duration: ui.reducedMotion ? 0 : 800 }}
        onerror={() => console.warn("backdrop failed to load", palette.backgroundSrc)}
      />
    {/if}
  {/key}
  <div class="mesh">
    <div class="blob b1"></div>
    <div class="blob b2"></div>
    <div class="blob b3"></div>
    <div class="blob b4"></div>
  </div>
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
    inset: -15%;
    width: 130%;
    height: 130%;
    object-fit: cover;
    filter: blur(var(--backdrop-blur)) saturate(1.2);
    opacity: 0.75;
    animation: drift 48s ease-in-out infinite alternate;
    will-change: transform;
  }
  .mesh {
    position: absolute;
    inset: 0;
    opacity: 0.55;
  }
  .blob {
    position: absolute;
    width: 75vmax;
    height: 75vmax;
    border-radius: 50%;
    will-change: transform;
  }
  .b1 {
    background: radial-gradient(closest-side, var(--c-vibrant), transparent);
    top: -30vmax;
    left: -20vmax;
    animation: float1 37s ease-in-out infinite alternate;
  }
  .b2 {
    background: radial-gradient(closest-side, var(--c-accent), transparent);
    bottom: -35vmax;
    right: -25vmax;
    animation: float2 43s ease-in-out infinite alternate;
  }
  .b3 {
    background: radial-gradient(closest-side, var(--c-muted), transparent);
    top: 10vmax;
    right: -30vmax;
    animation: float3 53s ease-in-out infinite alternate;
  }
  .b4 {
    background: radial-gradient(closest-side, var(--c-dominant), transparent);
    bottom: -30vmax;
    left: -15vmax;
    animation: float1 59s ease-in-out infinite alternate-reverse;
  }
  .dim {
    position: absolute;
    inset: 0;
    background: var(--c-dark-bg);
    opacity: var(--backdrop-dim);
  }

  @keyframes drift {
    from { transform: scale(1) translate(0, 0) rotate(0deg); }
    to { transform: scale(1.12) translate(-3%, 2%) rotate(4deg); }
  }
  @keyframes float1 {
    from { transform: translate(0, 0) scale(1); }
    to { transform: translate(25vw, 20vh) scale(1.25); }
  }
  @keyframes float2 {
    from { transform: translate(0, 0) scale(1.1); }
    to { transform: translate(-30vw, -15vh) scale(0.9); }
  }
  @keyframes float3 {
    from { transform: translate(0, 0) scale(0.9); }
    to { transform: translate(-20vw, 25vh) scale(1.2); }
  }

  @media (prefers-reduced-motion: reduce) {
    .art,
    .blob {
      animation: none;
    }
  }
</style>
