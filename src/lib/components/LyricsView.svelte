<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { Line, LyricsLoaded, Word } from "../types";
  import { player } from "../stores/player.svelte";
  import { onPlaybackFrame } from "../playbackFrames";
  import { lyrics } from "../stores/lyrics.svelte";
  import { activeLineIndex, INTERLUDE_MIN_MS, lyricTime, wordTimings } from "../timing";
  import { LineFiller } from "../wordFill";
  import Interlude from "./Interlude.svelte";
  import FillWord from "./FillWord.svelte";

  let { data, style }: { data: LyricsLoaded; style: "classic" | "minimal" | "wave" } = $props();

  /** Lines as displayed: a long intro becomes its own (empty) line. */
  const lines: Line[] = $derived.by(() => {
    const first = data.lines[0];
    if (first && first.start_ms >= INTERLUDE_MIN_MS) {
      return [{ start_ms: 0, end_ms: first.start_ms, text: "", words: null }, ...data.lines];
    }
    return data.lines;
  });
  const words: Word[][] = $derived(lines.map(wordTimings));

  let active = $state(-1);
  // DOM references; plain arrays because they are only read imperatively.
  const lineEls: (HTMLElement | undefined)[] = [];
  const wordEls: HTMLElement[][] = [];
  const dotsEls: (HTMLElement | undefined)[] = [];

  /** Attachment that stores `el` in `list[i]` while it is mounted. */
  function register<T extends HTMLElement>(list: (T | undefined)[], i: number) {
    return (el: T) => {
      list[i] = el;
      return () => {
        if (list[i] === el) list[i] = undefined;
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

  /** Lines further than this from the active one are not laid out at all. */
  const WINDOW = 3;
  const SCALE_NEAR = 0.55;
  const SCALE_FAR = 0.4;
  const GAP_PX = 28;

  let transforms: string[] = $state([]);

  function isBreak(i: number): boolean {
    const l = lines[i];
    return !!l && l.text.trim() === "" && l.end_ms - l.start_ms >= INTERLUDE_MIN_MS;
  }

  /**
   * Places the active line at the center and its neighbours above/below.
   * Only lines inside the window are measured and moved, so a line change
   * animates a handful of elements rather than the whole song.
   */
  function layout() {
    const h = (i: number) => lineEls[i]?.offsetHeight ?? 0;
    const out = transforms.slice(0, lines.length);
    const a = Math.max(0, active);
    const place = (i: number, center: number, scale: number) => {
      out[i] = `translate3d(0, ${center - h(i) / 2}px, 0) scale(${scale})`;
    };
    place(a, 0, 1);
    let up = -h(a) / 2;
    let down = h(a) / 2;
    for (let d = 1; d <= WINDOW; d++) {
      const s = d === 1 ? SCALE_NEAR : SCALE_FAR;
      const p = a - d;
      const n = a + d;
      if (p >= 0) {
        const c = up - GAP_PX - (h(p) * s) / 2;
        place(p, c, s);
        up = c - (h(p) * s) / 2;
      }
      if (n < lines.length) {
        const c = down + GAP_PX + (h(n) * s) / 2;
        place(n, c, s);
        down = c + (h(n) * s) / 2;
      }
    }
    transforms = out;
  }

  $effect(() => {
    void active;
    void lines;
    tick().then(layout);
  });

  // Per-frame work: only the active line's word fills and the interlude dots.
  const filler = new LineFiller();

  function frame(now: number) {
    const t = lyricTime(player.clock.position(now), lyrics.totalOffsetMs);
    const idx = activeLineIndex(lines, t);
    if (idx !== active) {
      filler.reset(wordEls[active]);
      active = idx;
    }
    if (idx < 0) return;

    // Drives two transforms per word only (see FillWord): no repaint per frame.
    filler.update(wordEls[idx], words[idx] ?? [], t);

    const dots = dotsEls[idx];
    if (dots) {
      const l = lines[idx];
      const progress = Math.min(1, Math.max(0, (t - l.start_ms) / (l.end_ms - l.start_ms)));
      dots.style.setProperty("--progress", progress.toFixed(3));
    }
  }

  onMount(() => {
    const stop = onPlaybackFrame(frame);
    const onResize = () => layout();
    window.addEventListener("resize", onResize);
    return () => {
      stop();
      window.removeEventListener("resize", onResize);
    };
  });

  function seekTo(line: Line) {
    player.seek(Math.max(0, line.start_ms + lyrics.totalOffsetMs));
  }
</script>

<div class="stage {style}">
  {#each lines as line, i (i)}
    {@const d = i - active}
    <!-- Only the visible window (plus one line either side, so lines slide
         in already laid out) is in the DOM; a song can have 100+ lines. -->
    {#if Math.abs(i - Math.max(0, active)) <= WINDOW + 1}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="line"
        class:active={d === 0}
        class:near={Math.abs(d) === 1}
        class:past={d < 0}
        class:far={Math.abs(d) > WINDOW}
        {@attach register(lineEls, i)}
        style:transform={transforms[i]}
        role="button"
        tabindex="-1"
        onclick={() => seekTo(line)}
      >
        {#if isBreak(i)}
          <div class="dots-wrap" {@attach register(dotsEls, i)}>
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

<style>
  .stage {
    position: absolute;
    inset: 0;
    overflow: hidden;
    --fill: var(--text);
    --unfilled: color-mix(in srgb, var(--text) 35%, transparent);
    --line-size: calc(clamp(2.4rem, 5.6cqw, 6.5rem) * var(--lyrics-scale, 1));
  }
  .line {
    position: absolute;
    top: 50%;
    left: 6cqw;
    right: 6cqw;
    text-align: center;
    font-size: var(--line-size);
    font-weight: 800;
    line-height: 1.12;
    letter-spacing: -0.02em;
    text-wrap: balance;
    color: var(--unfilled);
    transform-origin: center center;
    cursor: pointer;
    opacity: 0;
  }
  .line.far {
    display: none;
  }
  .classic .line,
  .wave .line {
    transition:
      transform 480ms cubic-bezier(0.34, 1.32, 0.64, 1),
      opacity 380ms ease;
  }
  .minimal .line {
    transition: opacity 200ms linear;
  }
  /* Only the lines that move or fill get their own GPU layer. */
  .line.active,
  .line.near {
    will-change: transform, opacity;
  }
  .line.active {
    opacity: 1;
  }
  .line.near {
    opacity: 0.5;
  }
  .line.past {
    color: var(--fill);
  }
  .line:not(.active):hover {
    opacity: 0.75;
  }

  .classic .active :global(.fw.on) {
    animation: pop 520ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .classic .active :global(.fw.glow .fill),
  .wave .active :global(.fw.glow .fill) {
    text-shadow: 0 0 0.22em color-mix(in srgb, var(--accent) 80%, transparent);
  }
  /* Wave: words wait slightly lowered and rise into place, one after
     another, as they are sung, so a wave runs along the line. */
  .wave .active :global(.fw) {
    transform: translate3d(0, 0.16em, 0) scale(0.94);
    transition: transform 650ms cubic-bezier(0.34, 1.56, 0.64, 1);
    will-change: transform;
  }
  .wave .active :global(.fw.on) {
    transform: none;
  }

  @keyframes pop {
    0% { transform: translateY(0); }
    35% { transform: translateY(-4px); }
    100% { transform: translateY(0); }
  }

  .dots-wrap {
    display: flex;
    justify-content: center;
    padding: 0.2em 0;
  }
</style>
