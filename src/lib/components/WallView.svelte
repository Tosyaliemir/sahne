<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { Line, LyricsLoaded, Word } from "../types";
  import { player } from "../stores/player.svelte";
  import { lyrics } from "../stores/lyrics.svelte";
  import { activeLineIndex, lyricTime, wordTimings } from "../timing";
  import { LineFiller } from "../wordFill";
  import FillWord from "./FillWord.svelte";

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

  const lines: Line[] = $derived(data.lines.filter((l) => l.text.trim() !== ""));
  const words: Word[][] = $derived(lines.map(wordTimings));
  /**
   * Uppercasing depends on language: Turkish maps i → İ, English i → I. The
   * UI language says nothing about the song's, so guess from letters that
   * only Turkish uses.
   */
  const lyricsLang = $derived(/[ığşİĞŞ]/.test(lines.map((l) => l.text).join(" ")) ? "tr" : "en");

  let active = $state(-1);
  const lineEls: (HTMLElement | undefined)[] = [];
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
      out[i] = `translate3d(0, ${bottom + GAP_PX}px, ${DEPTH_PX * 0.4}px)`;
      bottom += GAP_PX + h(i);
    }
    transforms = out;
  }

  $effect(() => {
    void active;
    void lines;
    tick().then(layout);
  });

  onMount(() => {
    let raf = requestAnimationFrame(function loop(now) {
      const t = lyricTime(player.clock.position(now), lyrics.totalOffsetMs);
      const idx = activeLineIndex(lines, t);
      if (idx !== active) {
        filler.reset(wordEls[active]);
        active = idx;
      }
      if (idx >= 0) filler.update(wordEls[idx], words[idx] ?? [], t);
      raf = requestAnimationFrame(loop);
    });
    const onResize = () => layout();
    window.addEventListener("resize", onResize);
    return () => {
      cancelAnimationFrame(raf);
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
          style:transform={transforms[i]}
          style:--age={Math.abs(d)}
          {@attach register(i)}
          role="button"
          tabindex="-1"
          onclick={() => player.seek(Math.max(0, line.start_ms + lyrics.totalOffsetMs))}
        >
          {#each words[i] as w, j (j)}
            <FillWord text={w.text} attach={registerWord(i, j)} />{" "}
          {/each}
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
    left: 9vw;
    right: 14vw;
    font-size: calc(clamp(2rem, 4.8vw, 5.6rem) * var(--lyrics-scale, 1));
    font-weight: 800;
    line-height: 1.02;
    letter-spacing: -0.015em;
    text-transform: uppercase;
    color: var(--unfilled);
    opacity: calc(1 - var(--age) * 0.2);
    transition:
      transform 620ms cubic-bezier(0.34, 1.25, 0.64, 1),
      opacity 500ms ease,
      color 500ms ease;
    cursor: pointer;
    animation: arrive 520ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .line.past {
    color: var(--fill);
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
</style>
