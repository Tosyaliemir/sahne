<!--
  Light flowing around the screen edges in the cover's ambient colors.
  A large conic gradient spins (transform only) behind a static frame-shaped
  mask, so the colors travel along the edges without repainting each frame.
-->
<script lang="ts">
  import { ui } from "../stores/ui.svelte";
</script>

<div class="edge" class:still={ui.reducedMotion} aria-hidden="true">
  <div class="ring"></div>
</div>

<style>
  .edge {
    position: fixed;
    inset: 0;
    pointer-events: none;
    z-index: 5;
    overflow: hidden;
    /* Union of a left/right and a top/bottom fade: a soft frame. */
    mask-image:
      linear-gradient(to right, #000, transparent 7%, transparent 93%, #000),
      linear-gradient(to bottom, #000, transparent 11%, transparent 89%, #000);
    animation: fade-in 1.2s ease;
  }
  .ring {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 160vmax;
    height: 160vmax;
    margin: -80vmax 0 0 -80vmax;
    background: conic-gradient(
      from 0deg,
      var(--c-amb-0),
      var(--c-amb-1),
      var(--c-amb-2),
      var(--c-amb-3),
      var(--c-amb-4),
      var(--c-amb-0)
    );
    opacity: 0.85;
    animation: spin 16s linear infinite;
    will-change: transform;
  }
  .still .ring {
    animation: none;
  }
  @keyframes spin {
    to {
      transform: rotate(1turn);
    }
  }
  @keyframes fade-in {
    from {
      opacity: 0;
    }
  }
  :global(html.idle) .ring {
    animation-play-state: paused;
  }
</style>
