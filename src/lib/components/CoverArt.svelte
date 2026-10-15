<script lang="ts">
  import { palette } from "../stores/palette.svelte";
  import { settings } from "../stores/settings.svelte";
  import { ui } from "../stores/ui.svelte";
  import { t } from "../i18n/index.svelte";
  import ElectricBorder from "./fx/ElectricBorder.svelte";
  import MetalCover from "./fx/MetalCover.svelte";

  /**
   * The big cover, wearing the effect chosen in settings, colored from the
   * cover's palette. Fills its parent, which sets the size.
   */
  let { radius = 14 }: { radius?: number } = $props();

  let failed = $state(false);
  $effect(() => {
    void palette.artSrc;
    failed = false;
  });

  const src = $derived(failed ? null : palette.artSrc);
  // Heavy shaders are pointless when the system asks for less motion.
  const chosen = $derived(ui.reducedMotion ? "yok" : settings.current.cover_effect);
  const accent = $derived(palette.palette.accent);

  /** Lightens a #rrggbb color toward white by `k` (0..1). */
  function lighten(hex: string, k: number): string {
    const n = parseInt(hex.slice(1), 16);
    const ch = (shift: number) => {
      const c = (n >> shift) & 255;
      return Math.round(c + (255 - c) * k);
    };
    return `rgb(${ch(16)}, ${ch(8)}, ${ch(0)})`;
  }
</script>

{#snippet plain()}
  <div class="plain" style:border-radius="{radius}px">
    {#if src}
      <img {src} alt="" onerror={() => (failed = true)} />
    {:else}
      <div class="placeholder" aria-label={t("track.noArt")}>♪</div>
    {/if}
  </div>
{/snippet}

{#if !src}
  {@render plain()}
{:else if chosen === "metal"}
  <MetalCover {src} textureSrc={palette.textureSrc} tint={lighten(accent, 0.45)} radius={radius + 4} />
{:else if chosen === "elektrik"}
  <ElectricBorder color={accent} {radius}>
    {@render plain()}
  </ElectricBorder>
{:else}
  {@render plain()}
{/if}

<style>
  .plain {
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: var(--surface);
    box-shadow: 0 30px 80px rgb(0 0 0 / 0.5);
  }
  img {
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
</style>
