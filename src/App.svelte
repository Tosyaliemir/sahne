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

  const HIDE_CONTROLS_MS = 2000;

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
    hideTimer = setTimeout(() => {
      if (!hoveringBar) ui.controlsVisible = false;
    }, HIDE_CONTROLS_MS);
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
      default:
        return;
    }
    bumpControls();
  }

  const data = $derived(lyrics.forTrack);
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
  {:else if data?.state === "synced" && data.lines.length > 0}
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
  {:else if data?.state === "plain" && data.plain}
    {#key data.track_id}
      <PlainLyrics text={data.plain} />
    {/key}
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
