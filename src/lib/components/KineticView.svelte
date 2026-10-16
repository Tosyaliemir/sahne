<script lang="ts">
  import { onMount } from "svelte";
  import { backOut, cubicIn } from "svelte/easing";
  import type { LyricsLoaded, Word } from "../types";
  import { player } from "../stores/player.svelte";
  import { onPlaybackFrame } from "../playbackFrames";
  import { lyrics } from "../stores/lyrics.svelte";
  import { activeLineIndex, interludeAt, lyricTime, wordProgress, wordTimings } from "../timing";
  import Interlude from "./Interlude.svelte";
  import FillWord from "./FillWord.svelte";

  let { data }: { data: LyricsLoaded } = $props();

  interface FlatWord extends Word {
    line: number;
  }

  const flat: FlatWord[] = $derived(
    data.lines.flatMap((line, i) => wordTimings(line).map((w) => ({ ...w, line: i }))),
  );

  /** Last word that has started at `t`, or -1. */
  function wordAt(t: number): number {
    let lo = 0;
    let hi = flat.length - 1;
    let found = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (flat[mid].start_ms <= t) {
        found = mid;
        lo = mid + 1;
      } else hi = mid - 1;
    }
    return found;
  }

  let current = $state(-1);
  let inBreak = $state(false);
  let lineIdx = $state(-1);
  let wordEl: HTMLElement | undefined;
  const registerWord = (el: HTMLElement) => {
    wordEl = el;
    return () => {
      if (wordEl === el) wordEl = undefined;
    };
  };
  let dotsEl: HTMLElement | undefined = $state();
  let lastP = -1;

  function frame(now: number) {
    const t = lyricTime(player.clock.position(now), lyrics.totalOffsetMs);
    const brk = interludeAt(data.lines, t);
    if (!!brk !== inBreak) inBreak = !!brk;
    if (brk && dotsEl) dotsEl.style.setProperty("--progress", brk.progress.toFixed(3));

    const idx = wordAt(t);
    if (idx !== current) {
      current = idx;
      lastP = -1;
    }
    const li = activeLineIndex(data.lines, t);
    if (li !== lineIdx) lineIdx = li;

    if (idx >= 0 && wordEl) {
      const p = Math.round(wordProgress(flat[idx], t) * 1000) / 1000;
      if (p !== lastP) {
        wordEl.style.setProperty("--p", String(p));
        lastP = p;
      }
    }
  }

  onMount(() =>
    onPlaybackFrame((now) => {
      frame(now);
    }),
  );

  // Alternate the tilt so consecutive words don't all lean the same way.
  function enter(_node: Element, { index }: { index: number }) {
    const dir = index % 2 === 0 ? -1 : 1;
    return {
      duration: 420,
      easing: backOut,
      css: (t: number, u: number) =>
        `opacity: ${Math.min(1, t * 1.6)}; transform: translate(-50%, -50%) scale(${0.55 + 0.45 * t}) rotate(${dir * 7 * u}deg);`,
    };
  }

  // Short and front-loaded so fast verses don't stack two words on screen.
  function leave(_node: Element) {
    return {
      duration: 160,
      easing: cubicIn,
      css: (t: number, u: number) =>
        `opacity: ${t * t}; transform: translate(-50%, calc(-50% - ${u * 0.35}em)) scale(${1 + 0.18 * u});`,
    };
  }

  const word = $derived(current >= 0 ? flat[current] : null);
  const fontSize = $derived(
    word
      ? `min(calc(clamp(4rem, 13cqw, 13rem) * var(--lyrics-scale, 1)), calc(170cqw / ${Math.max(4, [...word.text].length)}))`
      : "",
  );
</script>

<div class="stage">
  {#if inBreak}
    <div class="center" bind:this={dotsEl}><Interlude /></div>
  {:else if word}
    {#key current}
      <!-- `.active` turns on FillWord's GPU-composited fill. -->
      <span class="word active" style:font-size={fontSize} in:enter={{ index: current }} out:leave
        ><FillWord text={word.text} attach={registerWord} /></span
      >
    {/key}
  {/if}
  {#if lineIdx >= 0 && !inBreak}
    <p class="context">{data.lines[lineIdx].text}</p>
  {/if}
</div>

<style>
  .stage {
    position: absolute;
    inset: 0;
    overflow: hidden;
    --fill: var(--text);
    --unfilled: color-mix(in srgb, var(--text) 35%, transparent);
  }
  .word,
  .center {
    position: absolute;
    top: 46%;
    left: 50%;
    transform: translate(-50%, -50%);
  }
  .word {
    white-space: nowrap;
    font-weight: 800;
    letter-spacing: -0.03em;
    line-height: 1;
    color: var(--unfilled);
    will-change: transform, opacity;
  }
  /* Static glow on the bright copy: rasterized once, not every frame. */
  .word :global(.fill) {
    text-shadow: 0 0 0.12em color-mix(in srgb, var(--accent) 55%, transparent);
  }
  .context {
    position: absolute;
    left: 8cqw;
    right: 8cqw;
    bottom: 18%;
    margin: 0;
    text-align: center;
    font-size: clamp(1rem, 1.8cqw, 1.6rem);
    font-weight: 600;
    color: var(--text-dim);
  }
</style>
