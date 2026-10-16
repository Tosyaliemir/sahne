<script lang="ts">
  import { onMount } from "svelte";
  import { player } from "../stores/player.svelte";
  import { frames } from "../frames";
  import { formatTime } from "../format";
  import { t } from "../i18n/index.svelte";

  /** Pauses the per-frame fill update while the bar is hidden. */
  let { active = true }: { active?: boolean } = $props();

  const length = $derived(player.track?.length_ms ?? 0);
  let fillEl: HTMLElement | undefined = $state();

  onMount(() => {
    let last = -1;
    return frames.add({
      tick(now) {
        if (!fillEl || length <= 0) return;
        const f = Math.round(Math.min(1, player.positionNow(now) / length) * 2000) / 2000;
        if (f !== last) {
          fillEl.style.transform = `scaleX(${f})`;
          last = f;
        }
      },
      needsFrame: () => active && player.status === "playing",
    });
  });
  // Showing the bar again needs one frame to catch the fill up.
  $effect(() => {
    if (active) frames.wake();
  });

  function seekFromPointer(e: MouseEvent) {
    if (length <= 0) return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const x = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
    player.seek(x * length);
  }

  function seekFromKey(e: KeyboardEvent) {
    const step = e.key === "ArrowRight" ? 5000 : e.key === "ArrowLeft" ? -5000 : 0;
    if (step === 0 || length <= 0) return;
    e.preventDefault();
    e.stopPropagation();
    player.seek(Math.min(length, Math.max(0, player.positionNow(performance.now()) + step)));
  }
</script>

<div class="progress">
  <span class="time">{formatTime(player.positionMs)}</span>
  <div
    class="track"
    role="slider"
    tabindex="0"
    aria-label={t("controls.seek")}
    aria-valuemin={0}
    aria-valuemax={Math.round(length / 1000)}
    aria-valuenow={Math.round(player.positionMs / 1000)}
    aria-valuetext={formatTime(player.positionMs)}
    onclick={seekFromPointer}
    onkeydown={seekFromKey}
  >
    <div class="rail">
      <!-- scaleX keeps the per-frame update off the layout path -->
      <div class="fill" bind:this={fillEl}></div>
    </div>
  </div>
  <span class="time">{formatTime(length)}</span>
</div>

<style>
  .progress {
    display: grid;
    grid-template-columns: 3.5rem 1fr 3.5rem;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
  }
  .time {
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
    color: var(--text-dim);
  }
  .time:first-child {
    text-align: right;
  }
  .track {
    padding: 0.6rem 0;
    cursor: pointer;
  }
  .track:focus-visible {
    outline: 2px solid var(--text);
    outline-offset: 2px;
    border-radius: 4px;
  }
  .rail {
    height: 4px;
    border-radius: 2px;
    background: var(--rail);
    overflow: hidden;
    transition: height 150ms;
  }
  .track:hover .rail {
    height: 6px;
  }
  .fill {
    height: 100%;
    background: var(--text);
    transform-origin: left center;
    will-change: transform;
  }
</style>
