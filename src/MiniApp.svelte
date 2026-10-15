<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { invoke } from "@tauri-apps/api/core";
  import { player } from "./lib/stores/player.svelte";
  import { lyrics } from "./lib/stores/lyrics.svelte";
  import { palette } from "./lib/stores/palette.svelte";
  import { settings } from "./lib/stores/settings.svelte";
  import { ui } from "./lib/stores/ui.svelte";
  import { t } from "./lib/i18n/index.svelte";
  import { activeLineIndex, lyricTime, wordTimings } from "./lib/timing";
  import { LineFiller } from "./lib/wordFill";
  import { formatTime } from "./lib/format";
  import FillWord from "./lib/components/FillWord.svelte";
  import VolumeControl from "./lib/components/VolumeControl.svelte";

  let lineIdx = $state(-1);
  let progressEl: HTMLElement | undefined = $state();
  const wordEls: HTMLElement[] = [];
  const filler = new LineFiller();

  const data = $derived(lyrics.forTrack);
  const synced = $derived(data?.state === "synced" ? data.lines : null);
  const line = $derived(synced && lineIdx >= 0 ? synced[lineIdx] : null);
  const lineWords = $derived(line ? wordTimings(line) : []);
  // Next sung line, skipping instrumental gaps.
  const nextLine = $derived(
    synced && lineIdx >= -1 ? (synced.slice(lineIdx + 1).find((l) => l.text.trim() !== "") ?? null) : null,
  );
  const playing = $derived(player.status === "playing");

  function registerWord(j: number) {
    return (el: HTMLElement) => {
      wordEls[j] = el;
      return () => {
        if (wordEls[j] === el) delete wordEls[j];
      };
    };
  }

  onMount(() => {
    const stops: (() => void)[] = [];
    settings.start().then((s) => stops.push(s));
    player.start().then((s) => stops.push(s));
    lyrics.start().then((s) => stops.push(s));
    palette.start().then((s) => stops.push(s));

    let raf = requestAnimationFrame(function loop(now) {
      const pos = player.positionNow(now);
      const length = player.track?.length_ms ?? 0;
      if (progressEl && length > 0) progressEl.style.transform = `scaleX(${Math.min(1, pos / length)})`;
      if (synced) {
        const tl = lyricTime(pos, lyrics.totalOffsetMs);
        const i = activeLineIndex(synced, tl);
        if (i !== lineIdx) {
          filler.reset(wordEls);
          lineIdx = i;
        } else if (line) {
          filler.update(wordEls, lineWords, tl);
        }
      } else if (lineIdx !== -1) {
        lineIdx = -1;
      }
      raf = requestAnimationFrame(loop);
    });

    return () => {
      cancelAnimationFrame(raf);
      stops.forEach((s) => s());
    };
  });

  function expand() {
    invoke("set_mini_mode", { enabled: false }).catch((err) => console.warn("set_mini_mode failed", err));
  }

  function onKey(e: KeyboardEvent) {
    if (e.target instanceof HTMLElement && e.target.closest("[role=slider]")) return;
    if (e.key === " ") {
      e.preventDefault();
      player.playPause();
    } else if (e.key === "m" || e.key === "M" || e.key === "Escape") {
      expand();
    } else if (e.key === "ArrowLeft") {
      player.previous();
    } else if (e.key === "ArrowRight") {
      player.next();
    } else if ((e.key === "ArrowUp" || e.key === "ArrowDown") && player.volume !== null) {
      e.preventDefault();
      player.setVolume(player.volume + (e.key === "ArrowUp" ? 0.05 : -0.05));
    }
  }

  let artFailed = $state(false);
  $effect(() => {
    void palette.artSrc;
    artFailed = false;
  });
  const fadeMs = $derived(ui.reducedMotion ? 0 : 600);
</script>

<svelte:window onkeydown={onKey} />

<div class="mini" class:playing data-tauri-drag-region>
  <!-- Backdrop: the blurred cover drifting, with a palette glow on top. -->
  <div class="backdrop" aria-hidden="true">
    {#key palette.backgroundSrc}
      {#if palette.backgroundSrc}
        <img src={palette.backgroundSrc} alt="" transition:fade={{ duration: fadeMs }} />
      {/if}
    {/key}
    <div class="glow"></div>
  </div>

  <div class="cover" data-tauri-drag-region>
    {#key palette.artSrc}
      {#if palette.artSrc && !artFailed}
        <img src={palette.artSrc} alt="" onerror={() => (artFailed = true)} transition:fade={{ duration: fadeMs }} />
      {:else}
        <div class="placeholder">♪</div>
      {/if}
    {/key}
  </div>

  <div class="body" data-tauri-drag-region>
    {#if player.track}
      <div class="meta" data-tauri-drag-region>
        <span class="eq" aria-hidden="true"><i></i><i></i><i></i></span>
        <span class="title">{player.track.title}</span>
        <span class="artist">{player.track.artist}</span>
      </div>

      <div class="lyric" data-tauri-drag-region>
        {#key lineIdx}
          <p class="line active" data-tauri-drag-region>
            {#if line && line.text.trim()}
              {#each lineWords as w, j (j)}
                <FillWord text={w.text} attach={registerWord(j)} />{" "}
              {/each}
            {:else if synced}
              <span class="dots">• • •</span>
            {:else}
              <span class="muted">{player.track.album}</span>
            {/if}
          </p>
        {/key}
        {#if nextLine}
          <p class="next">{nextLine.text}</p>
        {/if}
      </div>

      <div class="bottom">
        <span class="time">{formatTime(player.positionMs)}</span>
        <div class="rail"><div class="fill" bind:this={progressEl}></div></div>
        <div class="buttons">
          <button onclick={player.previous} aria-label={t("controls.previous")} title={t("controls.previous")}>
            <svg viewBox="0 0 24 24"><path d="M7 6h2v12H7zM10.5 12 19 6v12z" /></svg>
          </button>
          <button class="main" onclick={player.playPause} aria-label={playing ? t("controls.pause") : t("controls.play")}>
            {#if playing}
              <svg viewBox="0 0 24 24"><path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" /></svg>
            {:else}
              <svg viewBox="0 0 24 24"><path d="M8 5v14l11-7z" /></svg>
            {/if}
          </button>
          <button onclick={player.next} aria-label={t("controls.next")} title={t("controls.next")}>
            <svg viewBox="0 0 24 24"><path d="M15 6h2v12h-2zM13.5 12 5 18V6z" /></svg>
          </button>
        </div>
        <div class="vol"><VolumeControl /></div>
      </div>
    {:else}
      <div class="meta"><span class="title">{t("status.noPlayer.title")}</span></div>
    {/if}
  </div>

  <button class="expand" onclick={expand} title={t("mini.expand")} aria-label={t("mini.expand")}>
    <svg viewBox="0 0 24 24"><path d="M14 3h7v7h-2V6.4l-5.3 5.3-1.4-1.4L17.6 5H14zM3 14h2v3.6l5.3-5.3 1.4 1.4L6.4 19H10v2H3z" /></svg>
  </button>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
  }
  .mini {
    --fill: var(--text);
    --unfilled: color-mix(in srgb, var(--text) 38%, transparent);
    position: fixed;
    inset: 6px;
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: 1rem;
    padding: 0.9rem 1rem 0.9rem 0.9rem;
    border-radius: 20px;
    overflow: hidden;
    background: var(--c-dark-bg);
    box-shadow:
      inset 0 0 0 1px color-mix(in srgb, var(--text) 12%, transparent),
      0 4px 14px rgb(0 0 0 / 0.35);
    cursor: grab;
  }

  .backdrop {
    position: absolute;
    inset: 0;
    z-index: 0;
    pointer-events: none;
  }
  .backdrop img {
    position: absolute;
    inset: -30%;
    width: 160%;
    height: 160%;
    object-fit: cover;
    opacity: 0.55;
    animation: drift 30s ease-in-out infinite alternate;
  }
  .glow {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(circle at 12% 30%, color-mix(in srgb, var(--c-amb-0) 55%, transparent), transparent 55%),
      radial-gradient(circle at 85% 100%, color-mix(in srgb, var(--c-amb-1) 45%, transparent), transparent 60%),
      linear-gradient(90deg, color-mix(in srgb, var(--c-dark-bg) 30%, transparent), color-mix(in srgb, var(--c-dark-bg) 72%, transparent));
  }

  .cover {
    position: relative;
    z-index: 1;
    width: 7.6rem;
    height: 7.6rem;
    border-radius: 12px;
    overflow: hidden;
    box-shadow: 0 10px 28px rgb(0 0 0 / 0.45);
    transition: transform 600ms cubic-bezier(0.22, 1, 0.36, 1);
    transform: scale(0.94);
  }
  .playing .cover {
    transform: scale(1);
    animation: breathe 5s ease-in-out infinite;
  }
  .cover img,
  .placeholder {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .placeholder {
    display: grid;
    place-items: center;
    background: var(--surface);
    color: var(--text-dim);
    font-size: 2.4rem;
  }

  .body {
    position: relative;
    z-index: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    height: 100%;
    justify-content: space-between;
  }
  .meta {
    display: flex;
    align-items: baseline;
    gap: 0.45rem;
    font-size: 0.82rem;
    white-space: nowrap;
    overflow: hidden;
    padding-right: 1.6rem;
  }
  .title {
    font-weight: 800;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
  }
  .artist {
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 2;
  }

  /* Tiny equalizer: bars move while playing, rest when paused. */
  .eq {
    display: inline-flex;
    align-items: flex-end;
    gap: 2px;
    height: 0.75rem;
    flex: none;
    align-self: center;
  }
  .eq i {
    width: 3px;
    height: 100%;
    border-radius: 1px;
    background: var(--accent);
    transform-origin: bottom;
    transform: scaleY(0.3);
  }
  .playing .eq i {
    animation: eq 0.9s ease-in-out infinite alternate;
  }
  .playing .eq i:nth-child(2) {
    animation-duration: 0.7s;
    animation-delay: -0.3s;
  }
  .playing .eq i:nth-child(3) {
    animation-duration: 1.1s;
    animation-delay: -0.6s;
  }

  .lyric {
    min-height: 3.1rem;
  }
  .line {
    margin: 0;
    font-size: 1.12rem;
    font-weight: 800;
    line-height: 1.2;
    color: var(--unfilled);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    animation: rise 380ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .line :global(.fw.glow .fill) {
    text-shadow: 0 0 0.25em color-mix(in srgb, var(--accent) 70%, transparent);
  }
  .next {
    margin: 0.15rem 0 0;
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--text-dim);
    opacity: 0.7;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .muted,
  .dots {
    color: var(--text-dim);
    font-weight: 600;
  }
  .dots {
    letter-spacing: 0.2em;
    animation: pulse 1.6s ease-in-out infinite;
  }

  .bottom {
    display: grid;
    grid-template-columns: auto 1fr auto auto;
    align-items: center;
    gap: 0.6rem;
  }
  .time {
    font-size: 0.7rem;
    font-variant-numeric: tabular-nums;
    color: var(--text-dim);
    min-width: 2.2rem;
  }
  .rail {
    height: 3px;
    border-radius: 2px;
    background: var(--rail);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--text);
    transform-origin: left center;
    transform: scaleX(0);
  }
  .buttons {
    display: flex;
    align-items: center;
    gap: 0.15rem;
  }
  .buttons button,
  .expand {
    display: grid;
    place-items: center;
    width: 1.9rem;
    height: 1.9rem;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--text);
    opacity: 0.8;
    cursor: pointer;
    transition: opacity 150ms, background-color 150ms, transform 150ms;
  }
  .buttons button:hover,
  .expand:hover {
    opacity: 1;
    background: var(--hover);
  }
  .buttons button:active {
    transform: scale(0.9);
  }
  .buttons button.main {
    width: 2.3rem;
    height: 2.3rem;
    background: var(--text);
    color: var(--c-dark-bg);
    opacity: 1;
  }
  svg {
    width: 1.05rem;
    height: 1.05rem;
    fill: currentColor;
  }
  /* The volume control is compact here: icon plus a short slider. */
  .vol :global(.track) {
    width: 4rem;
  }
  .vol :global(button) {
    width: 1.9rem;
    height: 1.9rem;
  }
  .expand {
    position: absolute;
    top: 0.45rem;
    right: 0.45rem;
    z-index: 2;
    width: 1.6rem;
    height: 1.6rem;
    opacity: 0;
  }
  .mini:hover .expand {
    opacity: 0.75;
  }
  .expand svg {
    width: 0.85rem;
    height: 0.85rem;
  }

  @keyframes drift {
    from { transform: translate3d(0, 0, 0) scale(1); }
    to { transform: translate3d(-6%, 4%, 0) scale(1.12); }
  }
  @keyframes breathe {
    0%, 100% { transform: scale(1); }
    50% { transform: scale(1.025); }
  }
  @keyframes eq {
    from { transform: scaleY(0.25); }
    to { transform: scaleY(1); }
  }
  @keyframes rise {
    from { opacity: 0; transform: translateY(8px); }
  }
  @keyframes pulse {
    50% { opacity: 0.4; }
  }
  @media (prefers-reduced-motion: reduce) {
    .backdrop img,
    .playing .cover,
    .playing .eq i,
    .line,
    .dots {
      animation: none;
    }
  }
</style>
