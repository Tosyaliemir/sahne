<script lang="ts">
  import type { LyricsLoaded } from "../types";
  import { player } from "../stores/player.svelte";
  import { activeLineIndex, lyricTime } from "../timing";

  let { data }: { data: LyricsLoaded } = $props();

  const active = $derived(activeLineIndex(data.lines, lyricTime(player.positionMs, data.offset_ms)));

  let lineEls: HTMLElement[] = $state([]);
  let shift = $state(0);

  // Layout is read only when the active line changes, not every frame.
  $effect(() => {
    const el = lineEls[Math.max(0, active)];
    shift = el ? -(el.offsetTop + el.offsetHeight / 2) : 0;
  });
</script>

<div class="viewport">
  <ol style:transform="translateY({shift}px)">
    {#each data.lines as line, i (i)}
      <li
        bind:this={lineEls[i]}
        class:active={i === active}
        class:past={i < active}
        class:break={!line.text}
      >
        <button onclick={() => player.seek(line.start_ms + data.offset_ms)}>
          {line.text || "♪"}
        </button>
      </li>
    {/each}
  </ol>
</div>

<style>
  .viewport {
    position: relative;
    height: 100%;
    overflow: hidden;
    mask-image: linear-gradient(transparent, #000 25%, #000 75%, transparent);
  }
  ol {
    position: absolute;
    top: 50%;
    left: 0;
    right: 0;
    margin: 0;
    padding: 0;
    list-style: none;
    transition: transform 450ms cubic-bezier(0.22, 1, 0.36, 1);
    will-change: transform;
  }
  li {
    padding: 0.35rem 0;
  }
  button {
    all: unset;
    cursor: pointer;
    font-size: 1.7rem;
    font-weight: 700;
    line-height: 1.25;
    color: var(--text);
    opacity: 0.35;
    transition: opacity 300ms;
  }
  .past button {
    opacity: 0.5;
  }
  .active button {
    opacity: 1;
  }
  button:hover {
    opacity: 0.8;
  }
  .break button {
    font-size: 1.2rem;
  }
</style>
