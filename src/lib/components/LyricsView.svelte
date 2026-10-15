<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { Line, LyricsLoaded, Word } from "../types";
  import { player } from "../stores/player.svelte";
  import {
    activeLineIndex,
    activeWordIndex,
    INTERLUDE_MIN_MS,
    lyricTime,
    wordProgress,
    wordTimings,
  } from "../timing";
  import Interlude from "./Interlude.svelte";

  let { data, style }: { data: LyricsLoaded; style: "klasik" | "sade" } = $props();

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
  let transforms: string[] = $state([]);

  const SCALE_NEAR = 0.55;
  const SCALE_FAR = 0.4;
  const GAP_PX = 28;

  function isBreak(i: number): boolean {
    const l = lines[i];
    return !!l && l.text.trim() === "" && l.end_ms - l.start_ms >= INTERLUDE_MIN_MS;
  }

  /** Places the active line at the center and its neighbours above/below. */
  function layout() {
    const h = (i: number) => lineEls[i]?.offsetHeight ?? 0;
    const out: string[] = new Array(lines.length);
    const a = Math.max(0, active);
    const place = (i: number, center: number, scale: number) => {
      out[i] = `translateY(${center - h(i) / 2}px) scale(${scale})`;
    };
    place(a, 0, 1);
    let up = -h(a) / 2;
    let down = h(a) / 2;
    for (let d = 1; d <= 2; d++) {
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
    // Far lines wait just outside the visible stack, ready to slide in.
    for (let i = 0; i < lines.length; i++) {
      if (!out[i]) out[i] = `translateY(${i < a ? up - 200 : down + 200}px) scale(${SCALE_FAR})`;
    }
    transforms = out;
  }

  $effect(() => {
    void active;
    void lines;
    tick().then(layout);
  });

  // Per-frame work: only the active line's word fills and the interlude dots.
  let lastProgress: number[] = [];
  let lastActiveWord = -1;

  function frame(now: number) {
    const t = lyricTime(player.clock.position(now), data.offset_ms);
    const idx = activeLineIndex(lines, t);
    if (idx !== active) {
      resetLine(active);
      active = idx;
      lastProgress = [];
      lastActiveWord = -1;
    }
    if (idx < 0) return;

    const els = wordEls[idx];
    const ws = words[idx];
    if (els && ws) {
      for (let j = 0; j < ws.length; j++) {
        const el = els[j];
        if (!el) continue;
        const p = Math.round(wordProgress(ws[j], t) * 1000) / 1000;
        if (p !== lastProgress[j]) {
          el.style.setProperty("--p", String(p));
          if (p > 0 && !(lastProgress[j] > 0)) el.classList.add("on");
          lastProgress[j] = p;
        }
      }
      const aw = activeWordIndex(ws, t);
      const singing = aw >= 0 && t < ws[aw].end_ms ? aw : -1;
      if (singing !== lastActiveWord) {
        if (lastActiveWord >= 0) els[lastActiveWord]?.classList.remove("glow");
        if (singing >= 0) els[singing]?.classList.add("glow");
        lastActiveWord = singing;
      }
    }

    const dots = dotsEls[idx];
    if (dots) {
      const l = lines[idx];
      const progress = Math.min(1, Math.max(0, (t - l.start_ms) / (l.end_ms - l.start_ms)));
      dots.style.setProperty("--progress", progress.toFixed(3));
    }
  }

  function resetLine(i: number) {
    for (const el of wordEls[i] ?? []) {
      el?.style.removeProperty("--p");
      el?.classList.remove("on", "glow");
    }
  }

  onMount(() => {
    let raf = requestAnimationFrame(function loop(now) {
      frame(now);
      raf = requestAnimationFrame(loop);
    });
    const onResize = () => layout();
    window.addEventListener("resize", onResize);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", onResize);
    };
  });

  function seekTo(line: Line) {
    player.seek(Math.max(0, line.start_ms + data.offset_ms));
  }
</script>

<div class="stage {style}">
  {#each lines as line, i (i)}
    {@const d = i - active}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
      class="line"
      class:active={d === 0}
      class:near={Math.abs(d) === 1}
      class:past={d < 0}
      class:hidden={Math.abs(d) > 1}
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
          <span class="w" {@attach registerWord(i, j)}>{w.text}</span>{" "}
        {/each}
      {/if}
    </div>
  {/each}
</div>

<style>
  .stage {
    position: absolute;
    inset: 0;
    overflow: hidden;
    --fill: var(--text);
    --unfilled: color-mix(in srgb, var(--text) 35%, transparent);
    --line-size: calc(clamp(2.4rem, 5.6vw, 6.5rem) * var(--lyrics-scale, 1));
  }
  .line {
    position: absolute;
    top: 50%;
    left: 6vw;
    right: 6vw;
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
  .klasik .line {
    transition:
      transform 480ms cubic-bezier(0.34, 1.32, 0.64, 1),
      opacity 380ms ease,
      color 380ms ease;
  }
  .sade .line {
    transition: opacity 200ms linear;
  }
  .line.active {
    opacity: 1;
  }
  .line.near {
    opacity: 0.5;
  }
  .line.past {
    color: var(--text);
  }
  .line.hidden {
    pointer-events: none;
  }
  .line:not(.active):hover {
    opacity: 0.75;
  }

  /* Word fill: a soft-edged gradient swept left to right by --p (0..1). */
  .w {
    display: inline-block;
    --p: 0;
  }
  .active .w {
    background-image: linear-gradient(
      90deg,
      var(--fill) calc(var(--p) * 125% - 25%),
      var(--unfilled) calc(var(--p) * 125%)
    );
    background-clip: text;
    -webkit-background-clip: text;
    color: transparent;
  }
  .klasik .active .w:global(.on) {
    animation: pop 520ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .klasik .active .w:global(.glow) {
    filter: drop-shadow(0 0 0.18em color-mix(in srgb, var(--accent) 75%, transparent));
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
