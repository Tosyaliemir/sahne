import { frames } from "./frames";
import { player } from "./stores/player.svelte";

/**
 * Registers a per-frame callback that runs while the track is playing, plus
 * one frame after any change (pause, seek, track change) so the final state
 * is drawn. Returns the cleanup.
 */
export function onPlaybackFrame(tick: (now: number) => void, extraNeeds?: () => boolean): () => void {
  return frames.add({
    tick,
    needsFrame: () => player.status === "playing" || (extraNeeds?.() ?? false),
  });
}
