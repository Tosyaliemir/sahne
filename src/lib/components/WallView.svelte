<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { Line, LyricsLoaded, Word } from "../types";
  import { player } from "../stores/player.svelte";
  import { onPlaybackFrame } from "../playbackFrames";
  import { lyrics } from "../stores/lyrics.svelte";
  import { activeLineIndex, INTERLUDE_MIN_MS, lyricTime, wordTimings } from "../timing";
  import { LineFiller } from "../wordFill";
  import FillWord from "./FillWord.svelte";
  import Interlude from "./Interlude.svelte";

  /**
   * "Duvar": the last few lines stacked into a wall of big type that hangs
   * at an angle in 3D and slowly sways. Each new line pushes the wall up;
   * older lines recede into the distance and fade.
   */
  let { data }: { data: LyricsLoaded } = $props();

  /** Lines kept on the wall behind the current one, and shown ahead of it. */
  const BEHIND = 3;
  const AHEAD = 1;
  const GAP_PX = 14;
  const DEPTH_PX = 70;
  /**
   * Where a line that just became visible starts, before layout has measured
   * it: well below the stack. Starting at the default (the center) made it
   * slide down across the active line.
   */
  const ENTER_TRANSFORM = "translate3d(0, 45cqh, 0)";

  /** A long empty line (or a long intro) is an instrumental break. */
  const isBreakLine = (l: Line) => l.text.trim() === "" && l.end_ms - l.start_ms >= INTERLUDE_MIN_MS;
  /**
   * Sung lines plus instrumental breaks, which show the same breathing dots
   * as the other styles. Short empty lines would only add gaps to the wall.
   */
  const lines: Line[] = $derived.by(() => {
    const first = data.lines[0];
    const all =
      first && first.start_ms >= INTERLUDE_MIN_MS
        ? [{ start_ms: 0, end_ms: first.start_ms, text: "", words: null }, ...data.lines]
        : data.lines;
    return all.filter((l) => l.text.trim() !== "" || isBreakLine(l));
  });
  const words: Word[][] = $derived(lines.map(wordTimings));
  /**
   * Uppercasing depends on language: Turkish maps i → İ, English i → I. The
   * UI language says nothing about the song's, so guess from letters that
   * only Turkish uses.
   */
  const lyricsLang = $derived(/[ığşİĞŞ]/.test(lines.map((l) => l.text).join(" ")) ? "tr" : "en");

  let active = $state(-1);
  const lineEls: (HTMLElement | undefined)[] = [];
  const dotsEls: (HTMLElement | undefined)[] = [];
  const wordEls: HTMLElement[][] = [];
  const filler = new LineFiller();
  let transforms: string[] = $state([]);

  function register(i: number) {
    return (el: HTMLElement) => {
      lineEls[i] = el;
      return () => {
        if (lineEls[i] === el) lineEls[i] = undefined;
      };
    };
  }
  function registerDots(i: number) {
    return (el: HTMLElement) => {
      dotsEls[i] = el;
      return () => {
        if (dotsEls[i] === el) dotsEls[i] = undefined;
      };
    };
  }
  function registerWord(i: number, j: number) {
    return (el: HTMLElement) => {
      (wordEls[i] ??= [])[j] = el;
      return () => {
        if (wordEls[i]?.[j] === el) delete wordEls[i][j];
      };
    };
  }

  const visible = (i: number) => i >= active - BEHIND && i <= active + AHEAD;

  /** Stacks visible lines upward from the active one, older ones further back. */
  function layout() {
    const out = transforms.slice(0, lines.length);
    const a = Math.max(0, active);
    const h = (i: number) => lineEls[i]?.offsetHeight ?? 0;
    out[a] = `translate3d(0, ${-h(a) / 2}px, 0)`;
    let top = -h(a) / 2;
    for (let d = 1; d <= BEHIND && a - d >= 0; d++) {
      const i = a - d;
      top -= GAP_PX + h(i);
      out[i] = `translate3d(0, ${top}px, ${-d * DEPTH_PX}px)`;
    }
    let bottom = h(a) / 2;
    for (let d = 1; d <= AHEAD && a + d < lines.length; d++) {
      const i = a + d;
      // Same depth as the active line, so perspective can't push it into it.
      out[i] = `translate3d(0, ${bottom + GAP_PX * 2}px, 0)`;
      bottom += GAP_PX * 2 + h(i);
    }
    transforms = out;
  }

  $effect(() => {
    void active;
    void lines;
    tick().then(layout);
  });

  onMount(() => {
    const stop = onPlaybackFrame((now) => {
      const t = lyricTime(player.clock.position(now), lyrics.totalOffsetMs);
      const idx = activeLineIndex(lines, t);
      if (idx !== active) {
        filler.reset(wordEls[active]);
        active = idx;
      }
      if (idx >= 0) filler.update(wordEls[idx], words[idx] ?? [], t);
      const dots = idx >= 0 ? dotsEls[idx] : undefined;
      if (dots) {
        const l = lines[idx];
        const progress = Math.min(1, Math.max(0, (t - l.start_ms) / (l.end_ms - l.start_ms)));
        dots.style.setProperty("--progress", progress.toFixed(3));
      }
    });
    const onResize = () => layout();
    window.addEventListener("resize", onResize);
    return () => {
      stop();
      window.removeEventListener("resize", onResize);
    };
  });
</script>

<div class="stage" lang={lyricsLang}>
  <div class="wall">
    {#each lines as line, i (i)}
      {#if visible(i)}
        {@const d = i - active}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="line"
          class:active={d === 0}
          class:past={d < 0}
          style:transform={transforms[i] ?? ENTER_TRANSFORM}
          style:--age={Math.abs(d)}
          {@attach register(i)}
          role="button"
          tabindex="-1"
          onclick={() => player.seek(Math.max(0, line.start_ms + lyrics.totalOffsetMs))}
        >
          {#if isBreakLine(line)}
            <div class="dots-wrap" {@attach registerDots(i)}>
              <Interlude />
            </div>
          {:else}
            {#each words[i] as w, j (j)}
              <FillWord text={w.text} attach={registerWord(i, j)} />{" "}
            {/each}
          {/if}
        </div>
      {/if}
    {/each}
  </div>
</div>

<style>
  .stage {
    position: absolute;
    inset: 0;
    overflow: hidden;
    perspective: 1100px;
    perspective-origin: 30% 45%;
    --fill: var(--text);
    --unfilled: color-mix(in srgb, var(--text) 32%, transparent);
  }
  .wall {
    position: absolute;
    inset: 0;
    transform-style: preserve-3d;
    animation: sway 26s ease-in-out infinite alternate;
    will-change: transform;
  }
  .line {
    position: absolute;
    top: 60%;
    left: 9cqw;
    right: 14cqw;
    font-size: calc(clamp(2rem, 4.8cqw, 5.6rem) * var(--lyrics-scale, 1));
    font-weight: 800;
    line-height: 1.02;
    letter-spacing: -0.015em;
    text-transform: uppercase;
    color: var(--unfilled);
    opacity: calc(1 - var(--age) * 0.2);
    transition:
      transform 620ms cubic-bezier(0.22, 1.08, 0.36, 1),
      opacity 500ms ease,
      color 500ms ease;
    cursor: pointer;
    animation: arrive 520ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .line.past {
    color: var(--fill);
  }
  .dots-wrap {
    display: flex;
    padding: 0.15em 0;
  }
  .line.active {
    opacity: 1;
  }
  .line.active :global(.fw.glow .fill) {
    text-shadow: 0 0 0.2em color-mix(in srgb, var(--accent) 75%, transparent);
  }
  @keyframes sway {
    from {
      transform: rotateY(-17deg) rotateX(7deg) translateZ(0);
    }
    to {
      transform: rotateY(-7deg) rotateX(3deg) translateZ(-40px);
    }
  }
  @keyframes arrive {
    from {
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .wall {
      animation: none;
      transform: rotateY(-12deg) rotateX(5deg);
    }
  }
  :global(html.idle) .wall {
    animation-play-state: paused;
  }
</style>
