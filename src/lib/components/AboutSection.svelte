<script lang="ts">
  import { t } from "../i18n/index.svelte";

  /**
   * Licence texts are large, so each one is its own chunk, loaded the first
   * time its section is opened. They are compiled in (no fetch), so the CSP
   * stays as strict as it is.
   */
  const sources = {
    notices: () => import("../../../THIRD_PARTY_NOTICES.md?raw"),
    license: () => import("../../../LICENSE?raw"),
    rust: () => import("../../assets/licenses/rust.txt?raw"),
    npm: () => import("../../assets/licenses/npm.txt?raw"),
  } as const;
  type Source = keyof typeof sources;

  let texts = $state<Partial<Record<Source, string>>>({});

  function load(key: Source, open: boolean) {
    if (!open || texts[key] !== undefined) return;
    sources[key]()
      .then((m) => (texts[key] = m.default))
      .catch((err) => console.warn(`licence text ${key} failed to load`, err));
  }
</script>

<section>
  <h3>{t("about.title")}</h3>
  <p class="line">Sahne <em>{__APP_VERSION__}</em></p>
  <p class="hint">{t("about.licensing")}</p>
  <p class="hint">{t("about.credits")}</p>
  <p class="hint">{t("about.spotify")}</p>
  <p class="hint">{t("about.lyrics")}</p>

  {#each ["notices", "license", "rust", "npm"] as const as key (key)}
    <details ontoggle={(e) => load(key, e.currentTarget.open)}>
      <summary>{t(`about.${key}` as const)}</summary>
      {#if texts[key] !== undefined}
        <pre>{texts[key]}</pre>
      {:else}
        <p class="hint">…</p>
      {/if}
    </details>
  {/each}
</section>

<style>
  h3 {
    margin: 1.6rem 0 0.6rem;
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
  }
  .line {
    margin: 0 0 0.4rem;
    font-size: 0.95rem;
  }
  em {
    font-style: normal;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
    margin-left: 0.35rem;
  }
  .hint {
    margin: 0.3rem 0 0;
    font-size: 0.82rem;
    line-height: 1.45;
    color: var(--text-dim);
  }
  details {
    margin-top: 0.6rem;
  }
  summary {
    cursor: pointer;
    font-size: 0.9rem;
    padding: 0.3rem 0;
  }
  pre {
    max-height: 18rem;
    overflow: auto;
    margin: 0.4rem 0 0;
    padding: 0.6rem;
    border-radius: 8px;
    background: var(--hover);
    font-size: 0.7rem;
    line-height: 1.4;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }
</style>
