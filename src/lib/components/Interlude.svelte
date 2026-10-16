<script lang="ts">
  // Purely presentational; see the comment below.
</script>

<!-- Three breathing dots for instrumental breaks. The parent sets --progress
     (0..1) and each dot fills in turn as the break runs out. -->
<div class="interlude" aria-hidden="true">
  <span style="--i: 0"></span>
  <span style="--i: 1"></span>
  <span style="--i: 2"></span>
</div>

<style>
  .interlude {
    display: inline-flex;
    gap: 0.45em;
    font-size: clamp(1.4rem, 2.6cqw, 2.6rem);
    animation: breathe 2.6s ease-in-out infinite;
  }
  span {
    width: 0.55em;
    height: 0.55em;
    border-radius: 50%;
    background: var(--text);
    /* Each dot owns a third of the break. */
    opacity: calc(0.25 + 0.75 * clamp(0, var(--progress, 0) * 3 - var(--i), 1));
  }
  @keyframes breathe {
    0%, 100% { transform: scale(0.88); }
    50% { transform: scale(1.12); }
  }
  @media (prefers-reduced-motion: reduce) {
    .interlude {
      animation: none;
    }
  }
</style>
