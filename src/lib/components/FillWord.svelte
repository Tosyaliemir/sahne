<script lang="ts">
  import type { Attachment } from "svelte/attachments";

  /**
   * A word that fills left to right as its --p custom property goes 0 → 1.
   *
   * The bright copy sits in a clipping box: the box slides right-to-left out of
   * the way while the copy slides the opposite way, so the text stays put and
   * only the visible part grows. Both are plain transforms, which the GPU
   * composites without repainting the (large) glyphs every frame. The fill is
   * only shown inside an `.active` line.
   */
  let { text, attach }: { text: string; attach: Attachment<HTMLElement> } = $props();
</script>

<span class="fw" {@attach attach}
  ><span class="base">{text}</span><span class="clip" aria-hidden="true"
    ><span class="fill">{text}</span></span
  ></span
>

<style>
  .fw {
    --p: 0;
    position: relative;
    display: inline-block;
    white-space: nowrap;
  }
  .clip {
    display: none;
  }
  /* Clip box and bright copy share one box (the word plus room for glyph
     overhang and glow), so equal and opposite percentages cancel exactly. */
  :global(.active) > .fw .clip {
    display: block;
    position: absolute;
    inset: -0.15em -0.3em;
    overflow: hidden;
    transform: translate3d(calc((var(--p) - 1) * 100%), 0, 0);
    /* Soft leading edge; it moves with the box, so it never repaints. */
    mask-image: linear-gradient(90deg, #000 calc(100% - 0.35em), transparent);
    will-change: transform;
  }
  :global(.active) > .fw:global(.done) .clip {
    mask-image: none;
  }
  .fill {
    position: absolute;
    inset: 0;
    padding: 0.15em 0.3em;
    box-sizing: border-box;
    color: var(--fill);
    transform: translate3d(calc((1 - var(--p)) * 100%), 0, 0);
    will-change: transform;
  }
</style>
