<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { player } from "./lib/stores/player.svelte";
  import { lyrics, OFFSET_STEP_MS } from "./lib/stores/lyrics.svelte";
  import { palette } from "./lib/stores/palette.svelte";
  import { ui } from "./lib/stores/ui.svelte";
  import { settings } from "./lib/stores/settings.svelte";
  import { t } from "./lib/i18n/index.svelte";
  import { startFrameStats } from "./lib/perf";
  import Background from "./lib/components/Background.svelte";
  import FlowBackground from "./lib/components/FlowBackground.svelte";
  import StatusScreen from "./lib/components/StatusScreen.svelte";
  import LyricsView from "./lib/components/LyricsView.svelte";
  import KineticView from "./lib/components/KineticView.svelte";
  import HarmanView from "./lib/components/HarmanView.svelte";
  import WallView from "./lib/components/WallView.svelte";
  import EdgeGlow from "./lib/components/EdgeGlow.svelte";
  import PlainLyrics from "./lib/components/PlainLyrics.svelte";
  import TrackHero from "./lib/components/TrackHero.svelte";
  import ControlBar from "./lib/components/ControlBar.svelte";
  import SettingsPanel from "./lib/components/SettingsPanel.svelte";


  /** WebGL missing or failing: fall back to the CSS backdrop. */
  let flowUnsupported = $state(false);

  let fullscreen = $state(false);
  const appWindow = getCurrentWindow();

  onMount(() => {
    const stops: (() => void)[] = [];
    settings.start().then((s) => stops.push(s));
    player.start().then((s) => stops.push(s));
    lyrics.start().then((s) => stops.push(s));
    palette.start().then((s) => stops.push(s));
    appWindow.isFullscreen().then((f) => (fullscreen = f)).catch(() => {});
    listen<boolean>("screensaver", (e) => {
      ui.screensaver = e.payload;
      if (e.payload) {
        ui.settingsOpen = false;
        ui.controlsVisible = false;
      }
      appWindow.isFullscreen().then((f) => (fullscreen = f)).catch(() => {});
    }).then((u) => stops.push(u));
    bumpControls();
    if (import.meta.env.DEV) stops.push(startFrameStats("frames"));
    return () => {
      stops.forEach((s) => s());
      clearTimeout(hideTimer);
    };
  });

  // --- auto-hiding controls -------------------------------------------------
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let hoveringBar = false;
  function bumpControls() {
    // The backend ends the screensaver on input; don't flash the bar meanwhile.
    if (ui.screensaver) return;
    ui.controlsVisible = true;
    clearTimeout(hideTimer);
    const secs = settings.current.controls_hide_secs;
    if (secs <= 0) return; // "never hide"
    hideTimer = setTimeout(() => {
      if (!hoveringBar) ui.controlsVisible = false;
    }, secs * 1000);
  }

  // --- toast ----------------------------------------------------------------
  let toast = $state<string | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  function showToast(text: string) {
    toast = text;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), 1500);
  }

  // --- actions ----------------------------------------------------------------
  async function toggleFullscreen(force?: boolean) {
    const next = force ?? !fullscreen;
    try {
      await appWindow.setFullscreen(next);
      fullscreen = next;
    } catch (err) {
      console.warn("setFullscreen failed", err);
    }
  }

  function nudge(delta: number) {
    lyrics.nudge(delta);
    const offset = lyrics.forTrack?.offset_ms;
    if (offset !== undefined) showToast(t("lyrics.offset", { value: (offset > 0 ? "+" : "") + offset }));
  }

  function cycleStyle() {
    const style = ui.nextStyle();
    settings.update({ lyrics_style: style });
    showToast(t("toast.style", { name: t(`style.${style}` as const) }));
  }

  function openMini() {
    invoke("set_mini_mode", { enabled: true }).catch((err) => console.warn("set_mini_mode failed", err));
  }

  function openSettings() {
    ui.settingsOpen = !ui.settingsOpen;
  }

  function onKey(e: KeyboardEvent) {
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    if (ui.settingsOpen) {
      if (e.key === "Escape" || e.key === ",") ui.settingsOpen = false;
      return;
    }
    if (e.target instanceof HTMLElement && e.target.closest("input, textarea, select, [role=slider]")) return;
    switch (e.key) {
      case " ":
        e.preventDefault();
        player.playPause();
        break;
      case "ArrowLeft":
        player.previous();
        break;
      case "ArrowRight":
        player.next();
        break;
      case "ArrowUp":
      case "ArrowDown":
        if (player.volume !== null) {
          e.preventDefault();
          player.setVolume(player.volume + (e.key === "ArrowUp" ? 0.05 : -0.05));
          showToast(t("toast.volume", { value: String(Math.round(Math.min(1, Math.max(0, player.volume)) * 100)) }));
        }
        break;
      case "[":
        nudge(-OFFSET_STEP_MS);
        break;
      case "]":
        nudge(OFFSET_STEP_MS);
        break;
      case "f":
      case "F":
        toggleFullscreen();
        break;
      case "Escape":
        if (fullscreen) toggleFullscreen(false);
        break;
      case "s":
      case "S":
        cycleStyle();
        break;
      case "m":
      case "M":
        openMini();
        break;
      case ",":
        openSettings();
        break;
      case "l":
      case "L":
        ui.setLyricsFocus(!ui.lyricsFocus);
        break;
      default:
        return;
    }
    bumpControls();
  }

  const data = $derived(lyrics.forTrack);

  let coverFailed = $state(false);
  $effect(() => {
    void palette.artSrc;
    coverFailed = false;
  });
  const style = $derived(ui.effectiveStyle);
</script>

<svelte:window onkeydown={onKey} onmousemove={bumpControls} onmousedown={bumpControls} />

{#if settings.current.backdrop_style === "akiskan" && !flowUnsupported}
  <FlowBackground onUnsupported={() => (flowUnsupported = true)} />
{:else}
  <Background mode={settings.current.backdrop_style === "akiskan" ? "bulanik" : settings.current.backdrop_style} />
{/if}
{#if settings.current.edge_glow && player.track}
  <EdgeGlow />
{/if}

<main class:hide-cursor={!ui.controlsVisible}>
  {#if !player.connected}
    <StatusScreen title={t("status.noPlayer.title")} body={t("status.noPlayer.body")} />
  {:else if !player.track}
    <StatusScreen
      title={t("status.idle.title")}
      body={t("status.idle.body", { player: player.playerName ?? "" })}
    />
  {:else if (data?.state === "synced" && data.lines.length > 0) || (data?.state === "plain" && data.plain)}
    <div class="layout" class:focus={ui.lyricsFocus}>
      <aside class="side" aria-hidden={ui.lyricsFocus}>
        <div class="cover">
          {#if palette.artSrc && !coverFailed}
            <img src={palette.artSrc} alt="" onerror={() => (coverFailed = true)} />
          {:else}
            <div class="placeholder">♪</div>
          {/if}
        </div>
        <h1>{player.track.title}</h1>
        <p class="artist">{player.track.artist}</p>
      </aside>
      <!-- In the split view a click anywhere on the lyrics enlarges them
           instead of seeking; once enlarged, clicking a line seeks. -->
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <section
        class="pane"
        class:switching={ui.switching}
        title={ui.lyricsFocus ? undefined : t("lyrics.expand")}
        onclickcapture={(e) => {
          if (ui.lyricsFocus) return;
          e.stopPropagation();
          ui.setLyricsFocus(true);
        }}
      >
        {#if data.state === "synced"}
    {#key data.track_id}
      {#if style === "kinetik"}
        <KineticView {data} />
      {:else if style === "harman"}
        <HarmanView {data} />
      {:else if style === "duvar"}
        <WallView {data} />
      {:else}
        <LyricsView {data} {style} />
      {/if}
    {/key}
        {:else}
    {#key data.track_id}
      <PlainLyrics text={data.plain ?? ""} />
    {/key}
        {/if}
      </section>
    </div>
  {:else if !data || data.state === "loading"}
    <TrackHero track={player.track} message={t("lyrics.loading")} />
  {:else if data.state === "instrumental"}
    <TrackHero track={player.track} message={t("lyrics.instrumental")} />
  {:else if data.state === "offline"}
    <TrackHero track={player.track} message={t("lyrics.offline")} />
  {:else if data.state === "error"}
    <TrackHero
      track={player.track}
      message={t("lyrics.error")}
      action={{ label: t("lyrics.retry"), run: () => lyrics.retry() }}
    />
  {:else}
    <TrackHero track={player.track} message={t("lyrics.notFound")} />
  {/if}

  {#if player.track && player.status !== "playing"}
    <div class="paused" role="status">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" /></svg>
      {t("status.paused")}
    </div>
  {/if}

  {#if toast}
    <div class="toast" role="status">{toast}</div>
  {/if}
</main>

{#if ui.settingsOpen}
  <SettingsPanel onClose={() => (ui.settingsOpen = false)} />
{/if}

<ControlBar
  visible={ui.controlsVisible && player.connected}
  {fullscreen}
  onToggleFullscreen={() => toggleFullscreen()}
  onCycleStyle={cycleStyle}
  onMini={openMini}
  onSettings={openSettings}
  onHoverChange={(h) => {
    hoveringBar = h;
    if (!h) bumpControls();
  }}
/>

<style>
  main {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
  }
  main.hide-cursor {
    cursor: none;
  }
  /* Split view: cover and title on the left, lyrics on the right. Focus
     view: the lyrics pane takes the whole screen and the cover steps away. */
  .layout {
    position: absolute;
    inset: 0;
  }
  .side {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 6vw;
    width: min(34vw, 60vh);
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding-bottom: 8vh;
    transition:
      opacity 420ms ease,
      transform 520ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .cover {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 14px;
    overflow: hidden;
    background: var(--surface);
    box-shadow: 0 30px 80px rgb(0 0 0 / 0.5);
  }
  .cover img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .placeholder {
    display: grid;
    place-items: center;
    height: 100%;
    font-size: 6rem;
    color: var(--text-dim);
  }
  .side h1 {
    margin: 1.6rem 0 0;
    font-size: clamp(1.6rem, 2.6vw, 2.8rem);
    font-weight: 800;
    line-height: 1.1;
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;
  }
  .side .artist {
    margin: 0.4rem 0 0;
    font-size: clamp(1rem, 1.4vw, 1.4rem);
    font-weight: 600;
    opacity: 0.8;
  }
  .pane {
    position: absolute;
    top: 0;
    bottom: 0;
    left: calc(6vw + min(34vw, 60vh) + 3vw);
    right: 2vw;
    container-type: size;
    cursor: zoom-in;
    transition: opacity 220ms ease;
  }
  .pane.switching {
    opacity: 0;
  }
  .focus .side {
    opacity: 0;
    transform: translateX(-3vw) scale(0.94);
    pointer-events: none;
  }
  .focus .pane {
    left: 0;
    right: 0;
    cursor: auto;
  }
  /* The views' own hover states would suggest a seek that won't happen. */
  .layout:not(.focus) .pane > :global(*) {
    pointer-events: none;
  }
  @media (max-aspect-ratio: 1/1) {
    .layout:not(.focus) .side {
      display: none;
    }
    .layout:not(.focus) .pane {
      left: 0;
      right: 0;
    }
  }
  .paused {
    position: fixed;
    top: 1.5rem;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.45rem 1rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--c-dark-bg) 60%, transparent);
    backdrop-filter: blur(16px);
    font-size: 0.9rem;
    font-weight: 600;
    animation: fade-in 300ms ease;
  }
  .paused svg {
    width: 1rem;
    height: 1rem;
    fill: currentColor;
  }
  .toast {
    position: fixed;
    top: 4.5rem;
    left: 50%;
    transform: translateX(-50%);
    padding: 0.5rem 1.1rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--c-dark-bg) 70%, transparent);
    backdrop-filter: blur(16px);
    font-size: 0.9rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    animation: fade-in 200ms ease;
  }
  @keyframes fade-in {
    from { opacity: 0; }
  }
</style>
