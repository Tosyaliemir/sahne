<script lang="ts">
  let { title, body }: { title: string; body: string } = $props();
</script>

<section class="status" aria-live="polite">
  <svg class="mark" viewBox="0 0 48 48" aria-hidden="true">
    <!-- A small stage with a spotlight: Sahne's placeholder mark. -->
    <path d="M24 4 8 30h32z" opacity="0.25" />
    <rect x="6" y="32" width="36" height="6" rx="3" />
    <circle cx="24" cy="6" r="3" />
  </svg>
  <h1>{title}</h1>
  <p>{body}</p>
</section>

<style>
  .status {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    max-width: 26rem;
    text-align: center;
  }
  .mark {
    width: 3.5rem;
    fill: var(--text);
    opacity: 0.8;
    margin-bottom: 0.5rem;
  }
  h1 {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 800;
  }
  p {
    margin: 0;
    color: var(--text-dim);
    line-height: 1.5;
  }
</style>
