<script lang="ts">
  import { onMount, untrack } from "svelte";
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
  import { Lifecycle } from "./lib/lifecycle";
  import { frames } from "./lib/frames";
  import Background from "./lib/components/Background.svelte";
  import FlowBackground from "./lib/components/FlowBackground.svelte";
  import StatusScreen from "./lib/components/StatusScreen.svelte";
  import LyricsView from "./lib/components/LyricsView.svelte";
  import KineticView from "./lib/components/KineticView.svelte";
  import BlendView from "./lib/components/BlendView.svelte";
  import WallView from "./lib/components/WallView.svelte";
  import EdgeGlow from "./lib/components/EdgeGlow.svelte";
  import PlainLyrics from "./lib/components/PlainLyrics.svelte";
  import TrackHero from "./lib/components/TrackHero.svelte";
  import ControlBar from "./lib/components/ControlBar.svelte";
  import SettingsPanel from "./lib/components/SettingsPanel.svelte";
  import CoverArt from "./lib/components/CoverArt.svelte";
  import LyricsAnnouncer from "./lib/components/LyricsAnnouncer.svelte";

  /** WebGL missing or failing: fall back to the CSS backdrop. */
  let flowUnsupported = $state(false);

  let fullscreen = $state(false);
  const appWindow = getCurrentWindow();

  onMount(() => {
    const life = new Lifecycle();
    life.add(settings.start());
    life.add(player.start());
    life.add(lyrics.start());
    life.add(palette.start());
    appWindow
      .isFullscreen()
      .then((f) => (fullscreen = f))
      .catch(() => {});
    life.add(
      listen<boolean>("screensaver", (e) => {
        ui.screensaver = e.payload;
        if (e.payload) {
          ui.settingsOpen = false;
          ui.controlsVisible = false;
        }
        appWindow
          .isFullscreen()
          .then((f) => (fullscreen = f))
          .catch(() => {});
      }),
    );
    // Hidden or minimised: draw nothing. WebKitGTK doesn't always report
    // minimising through visibilityState, so ask the window as well.
    let minimized = false;
    const updateHidden = () => {
      const hidden = document.visibilityState === "hidden" || minimized;
      ui.windowHidden = hidden;
      frames.setHidden(hidden);
    };
    document.addEventListener("visibilitychange", updateHidden);
    life.add(() => document.removeEventListener("visibilitychange", updateHidden));
    life.add(
      appWindow.onResized(async () => {
        minimized = await appWindow.isMinimized().catch(() => false);
        updateHidden();
      }),
    );
    updateHidden();
    bumpControls();
    if (import.meta.env.DEV) life.add(startFrameStats("frames"));
    return () => {
      life.dispose();
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

  // The timer above may have started with the default before settings loaded.
  $effect(() => {
    void settings.current.controls_hide_secs;
    untrack(bumpControls);
  });

  // Mouse moves arrive at display rate; resetting timers that often is waste.
  let lastMove = 0;
  function onMouseMove(e: MouseEvent) {
    if (e.timeStamp - lastMove < MOVE_THROTTLE_MS) return;
    lastMove = e.timeStamp;
    bumpControls();
  }
  const MOVE_THROTTLE_MS = 100;

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
          showToast(
            t("toast.volume", { value: String(Math.round(Math.min(1, Math.max(0, player.volume)) * 100)) }),
          );
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

  // Ambient CSS animations (drifting backdrop, edge light...) pause with
  // playback and while hidden; see `html.idle` rules in the components.
  $effect(() => {
    document.documentElement.classList.toggle("idle", ui.windowHidden || player.status !== "playing");
  });

  const style = $derived(ui.effectiveStyle);
</script>

<svelte:window onkeydown={onKey} onmousemove={onMouseMove} onmousedown={bumpControls} />

{#if settings.current.backdrop_style === "fluid" && !flowUnsupported}
  <FlowBackground onUnsupported={() => (flowUnsupported = true)} />
{:else}
  <Background
    mode={settings.current.backdrop_style === "fluid" ? "blurred" : settings.current.backdrop_style}
  />
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
          <CoverArt active={!ui.lyricsFocus} />
        </div>
        <h1>{player.track.title}</h1>
        <p class="artist">{player.track.artist}</p>
      </aside>
      <!-- In the split view a click anywhere on the lyrics enlarges them
           instead of seeking; once enlarged, clicking a line seeks. -->
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
            {#if style === "kinetic"}
              <KineticView {data} />
            {:else if style === "blend"}
              <BlendView {data} />
            {:else if style === "wall"}
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
      {#if player.status === "stopped"}
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6h12v12H6z" /></svg>
        {t("status.stopped")}
      {:else}
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" /></svg>
        {t("status.paused")}
      {/if}
    </div>
  {/if}

  {#if toast}
    <div class="toast" role="status">{toast}</div>
  {/if}
</main>

<LyricsAnnouncer />

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
  /* While the backdrop moves, a backdrop blur is recomputed every frame;
     a denser fill looks close and costs nothing. (Paused, it's static.) */
  :global(html:not(.idle)) .toast {
    backdrop-filter: none;
    background: color-mix(in srgb, var(--c-dark-bg) 86%, transparent);
  }
  @keyframes fade-in {
    from {
      opacity: 0;
    }
  }
</style>
