<script lang="ts">
  import { player } from "../stores/player.svelte";
  import { palette } from "../stores/palette.svelte";
  import { ui } from "../stores/ui.svelte";
  import { t } from "../i18n/index.svelte";
  import Controls from "./Controls.svelte";
  import ProgressBar from "./ProgressBar.svelte";
  import VolumeControl from "./VolumeControl.svelte";

  let {
    visible,
    fullscreen,
    onToggleFullscreen,
    onCycleStyle,
    onMini,
    onSettings,
    onHoverChange,
  }: {
    visible: boolean;
    fullscreen: boolean;
    onToggleFullscreen: () => void;
    onCycleStyle: () => void;
    onMini: () => void;
    onSettings: () => void;
    onHoverChange: (hovering: boolean) => void;
  } = $props();

  let artFailed = $state(false);
  $effect(() => {
    void palette.artSrc;
    artFailed = false;
  });
</script>

<div
  class="bar"
  class:visible
  role="toolbar"
  tabindex="-1"
  onmouseenter={() => onHoverChange(true)}
  onmouseleave={() => onHoverChange(false)}
>
  {#if player.track}
    <div class="track">
      <button
        class="thumb"
        class:active={ui.lyricsFocus}
        onclick={() => ui.setLyricsFocus(false)}
        title={t("controls.showCover")}
        aria-label={t("controls.showCover")}
        disabled={!ui.lyricsFocus}
      >
        {#if palette.artSrc && !artFailed}
          <img src={palette.artSrc} alt="" onerror={() => (artFailed = true)} />
        {:else}
          <div class="thumb-placeholder">♪</div>
        {/if}
      </button>
      <div class="meta">
        <span class="title">{player.track.title}</span>
        <span class="artist">{player.track.artist}</span>
      </div>
    </div>
    <div class="center">
      <Controls />
      <ProgressBar active={visible} />
    </div>
  {/if}
  <div class="tools">
    <VolumeControl />
    <span class="sep" aria-hidden="true"></span>
    <button onclick={onCycleStyle} title={t("controls.style")} aria-label={t("controls.style")}>
      <span class="style-name">{t(`style.${ui.effectiveStyle}` as const)}</span>
    </button>
    <button onclick={onMini} title={t("controls.mini")} aria-label={t("controls.mini")}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 5h18v14H3zm2 2v10h14V7zm6 5h6v4h-6z" /></svg>
    </button>
    <button onclick={onToggleFullscreen} title={t("controls.fullscreen")} aria-label={t("controls.fullscreen")}>
      {#if fullscreen}
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 3v5H3v2h7V3zm8 0h-2v7h7V8h-5zM3 14v2h5v5h2v-7zm11 0v7h2v-5h5v-2z" /></svg>
      {:else}
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 3h7v2H5v5H3zm11 0h7v7h-2V5h-5zM3 14h2v5h5v2H3zm16 0h2v7h-7v-2h5z" /></svg>
      {/if}
    </button>
    <button onclick={onSettings} title={t("controls.settings")} aria-label={t("controls.settings")}>
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M19.4 13a7.5 7.5 0 0 0 0-2l2.1-1.6-2-3.5-2.5 1a7.4 7.4 0 0 0-1.7-1L14.9 3h-4l-.4 2.9a7.4 7.4 0 0 0-1.7 1l-2.5-1-2 3.5L6.4 11a7.5 7.5 0 0 0 0 2l-2.1 1.6 2 3.5 2.5-1a7.4 7.4 0 0 0 1.7 1l.4 2.9h4l.4-2.9a7.4 7.4 0 0 0 1.7-1l2.5 1 2-3.5zM12.9 15.5a3.5 3.5 0 1 1 0-7 3.5 3.5 0 0 1 0 7z" />
      </svg>
    </button>
  </div>
</div>

<style>
  .bar {
    position: fixed;
    left: 1.25rem;
    right: 1.25rem;
    bottom: 1.25rem;
    display: grid;
    /* The tools never shrink below their content (they used to overlap); the
       track info gives way instead. */
    grid-template-columns: minmax(0, 1fr) minmax(14rem, 34rem) minmax(max-content, 1fr);
    align-items: center;
    gap: 1.5rem;
    padding: 0.75rem 1.25rem;
    border-radius: 18px;
    background: color-mix(in srgb, var(--c-dark-bg) 72%, transparent);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.3);
    opacity: 0;
    transform: translateY(1.5rem);
    pointer-events: none;
    transition:
      opacity 300ms ease,
      transform 400ms cubic-bezier(0.22, 1, 0.36, 1);
    z-index: 10;
  }
  /* The backdrop blur re-renders whatever moves behind it every frame, so
     it only exists while the bar is shown. */
  .bar.visible {
    backdrop-filter: blur(16px);
    opacity: 1;
    transform: translateY(0);
    pointer-events: auto;
  }
  /* While the backdrop moves the blur is recomputed every frame (costly on
     integrated GPUs); a denser fill looks close. Paused, it's static. */
  :global(html:not(.idle)) .bar.visible {
    backdrop-filter: none;
    background: color-mix(in srgb, var(--c-dark-bg) 86%, transparent);
  }
  .track {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    min-width: 0;
  }
  .thumb {
    flex: none;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: none;
    cursor: default;
    transition: transform 200ms cubic-bezier(0.34, 1.56, 0.64, 1), box-shadow 200ms ease;
  }
  .thumb.active {
    cursor: zoom-out;
  }
  .thumb.active:hover {
    transform: scale(1.08);
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.4);
  }
  .thumb:focus-visible {
    outline: 2px solid var(--text);
    outline-offset: 2px;
  }
  img,
  .thumb-placeholder {
    display: block;
    width: 3.25rem;
    height: 3.25rem;
    border-radius: 8px;
    object-fit: cover;
    flex: none;
  }
  .thumb-placeholder {
    display: grid;
    place-items: center;
    background: var(--surface);
    color: var(--text-dim);
  }
  .meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .title,
  .artist {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title {
    font-weight: 700;
  }
  .artist {
    font-size: 0.85rem;
    color: var(--text-dim);
  }
  .center {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    align-items: stretch;
  }
  .center :global(.controls) {
    gap: 0.75rem;
  }
  .center :global(.controls button) {
    width: 2.4rem;
    height: 2.4rem;
  }
  .center :global(.controls button.main) {
    width: 2.8rem;
    height: 2.8rem;
  }
  .tools {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.35rem;
  }
  .sep {
    width: 1px;
    height: 1.4rem;
    margin: 0 0.35rem;
    background: var(--rail);
  }
  .tools > :global(*) {
    flex: none;
  }
  .tools button {
    display: grid;
    place-items: center;
    min-width: 2.4rem;
    height: 2.4rem;
    padding: 0 0.5rem;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: var(--text);
    opacity: 0.8;
    cursor: pointer;
    font: inherit;
  }
  .tools button:hover {
    opacity: 1;
    background: var(--hover);
  }
  .tools svg {
    width: 1.25rem;
    height: 1.25rem;
    fill: currentColor;
  }
  @media (max-width: 1000px) {
    .bar {
      gap: 1rem;
    }
    .meta {
      display: none;
    }
    .tools :global(.volume .track) {
      width: 4rem;
    }
  }
  .style-name {
    font-size: 0.8rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
</style>
