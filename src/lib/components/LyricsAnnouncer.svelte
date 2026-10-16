<script lang="ts">
  import { player } from "../stores/player.svelte";
  import { lyrics } from "../stores/lyrics.svelte";
  import { settings } from "../stores/settings.svelte";
  import { activeLineIndex, lyricTime } from "../timing";

  /**
   * Screen-reader announcements of the current lyric line (A2), opt-in via
   * Settings. Polite and throttled: at most one line every two seconds, so
   * fast passages don't flood the reader; skipped lines are not queued.
   */
  const MIN_GAP_MS = 2000;
  const POLL_MS = 500;

  let text = $state("");

  $effect(() => {
    if (!settings.current.announce_lyrics) {
      text = "";
      return;
    }
    let lastIndex = -1;
    let lastAt = -Infinity;
    const timer = setInterval(() => {
      const data = lyrics.forTrack;
      if (!data || data.state !== "synced" || player.status !== "playing") return;
      const now = performance.now();
      const i = activeLineIndex(data.lines, lyricTime(player.clock.position(now), lyrics.totalOffsetMs));
      if (i < 0 || i === lastIndex || now - lastAt < MIN_GAP_MS) return;
      lastIndex = i;
      const line = data.lines[i].text.trim();
      if (!line) return;
      text = line;
      lastAt = now;
    }, POLL_MS);
    return () => clearInterval(timer);
  });
</script>

<div class="announcer" aria-live="polite" aria-atomic="true">{text}</div>

<style>
  .announcer {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
