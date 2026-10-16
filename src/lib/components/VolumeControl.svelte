<script lang="ts">
  import { player } from "../stores/player.svelte";
  import { t } from "../i18n/index.svelte";

  /** Speaker button (mute toggle) plus a slider; the wheel also adjusts it. */
  let restoreTo = 0.5;
  let dragging = $state(false);
  let railEl: HTMLElement | undefined = $state();

  const v = $derived(player.volume ?? 0);
  const icon = $derived(v === 0 ? "mute" : v < 0.34 ? "low" : v < 0.67 ? "mid" : "high");

  function toggleMute() {
    if (v > 0) {
      restoreTo = v;
      player.setVolume(0);
    } else {
      player.setVolume(restoreTo || 0.5);
    }
  }

  function fromPointer(e: PointerEvent) {
    if (!railEl) return;
    const r = railEl.getBoundingClientRect();
    player.setVolume((e.clientX - r.left) / r.width);
  }

  function onKey(e: KeyboardEvent) {
    const step =
      e.key === "ArrowRight" || e.key === "ArrowUp"
        ? 0.05
        : e.key === "ArrowLeft" || e.key === "ArrowDown"
          ? -0.05
          : 0;
    if (!step) return;
    e.preventDefault();
    e.stopPropagation();
    player.setVolume(v + step);
  }
</script>

{#if player.volume !== null}
  <div
    class="volume"
    class:dragging
    onwheel={(e) => {
      e.preventDefault();
      player.setVolume(v + (e.deltaY < 0 ? 0.05 : -0.05));
    }}
  >
    <button onclick={toggleMute} aria-label={t("controls.mute")} title={t("controls.mute")}>
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M4 9h4l5-4v14l-5-4H4z" />
        {#if icon === "mute"}
          <path
            d="M16 9l5 6M21 9l-5 6"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            fill="none"
          />
        {:else}
          <path
            d="M16 9.5a3.5 3.5 0 0 1 0 5"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            fill="none"
          />
          {#if icon !== "low"}
            <path
              d="M18.5 7a7 7 0 0 1 0 10"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              fill="none"
              opacity={icon === "high" ? 1 : 0.35}
            />
          {/if}
        {/if}
      </svg>
    </button>
    <div
      class="track"
      role="slider"
      tabindex="0"
      aria-label={t("controls.volume")}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(v * 100)}
      onpointerdown={(e) => {
        dragging = true;
        (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
        fromPointer(e);
      }}
      onpointermove={(e) => dragging && fromPointer(e)}
      onpointerup={() => (dragging = false)}
      onpointercancel={() => (dragging = false)}
      onkeydown={onKey}
    >
      <div class="rail" bind:this={railEl}>
        <div class="fill" style:transform="scaleX({v})"></div>
        <div class="knob" style:left="{v * 100}%"></div>
      </div>
      <span class="tip">{Math.round(v * 100)}</span>
    </div>
  </div>
{/if}

<style>
  .volume {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }
  button {
    display: grid;
    place-items: center;
    width: 2.2rem;
    height: 2.2rem;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--text);
    opacity: 0.8;
    cursor: pointer;
    transition:
      opacity 150ms,
      background-color 150ms;
  }
  button:hover {
    opacity: 1;
    background: var(--hover);
  }
  svg {
    width: 1.25rem;
    height: 1.25rem;
    fill: currentColor;
  }
  .track {
    position: relative;
    width: 6.5rem;
    padding: 0.7rem 0;
    cursor: pointer;
    touch-action: none;
  }
  .track:focus-visible {
    outline: 2px solid var(--text);
    outline-offset: 2px;
    border-radius: 4px;
  }
  .rail {
    position: relative;
    height: 4px;
    border-radius: 2px;
    background: var(--rail);
  }
  .fill {
    position: absolute;
    inset: 0;
    border-radius: 2px;
    background: var(--text);
    transform-origin: left center;
  }
  .knob {
    position: absolute;
    top: 50%;
    width: 12px;
    height: 12px;
    margin: -6px 0 0 -6px;
    border-radius: 50%;
    background: var(--text);
    box-shadow: 0 2px 6px rgb(0 0 0 / 0.35);
    transform: scale(0);
    transition: transform 150ms ease;
  }
  .track:hover .knob,
  .dragging .knob {
    transform: scale(1);
  }
  .tip {
    position: absolute;
    bottom: 100%;
    left: 50%;
    transform: translate(-50%, 4px);
    padding: 0.15rem 0.45rem;
    border-radius: 6px;
    background: color-mix(in srgb, var(--c-dark-bg) 85%, transparent);
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
    opacity: 0;
    pointer-events: none;
    transition:
      opacity 150ms,
      transform 150ms;
  }
  .track:hover .tip,
  .dragging .tip {
    opacity: 1;
    transform: translate(-50%, 0);
  }
</style>
