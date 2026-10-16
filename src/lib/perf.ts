import { invoke } from "@tauri-apps/api/core";
import { frames } from "./frames";

/**
 * Development aid: every 10 s, logs how many frames were actually drawn and
 * their pacing (95th-percentile frame time, frames slower than 25 ms). It
 * observes the shared scheduler, so it never causes frames itself: an idle
 * window should log "0 frames".
 */
export function startFrameStats(label: string): () => void {
  const WINDOW_MS = 10_000;
  let deltas: number[] = [];
  let last = 0;
  let count = 0;
  const stop = frames.observe((now) => {
    count++;
    // Gaps over 100 ms are the loop sleeping, not slow frames.
    if (last && now - last < 100) deltas.push(now - last);
    last = now;
  });
  const timer = setInterval(() => {
    let message = `${label}: ${count} frames in 10 s`;
    if (deltas.length > 0) {
      const sorted = [...deltas].sort((a, b) => a - b);
      const p95 = sorted[Math.floor(sorted.length * 0.95)];
      const slow = deltas.filter((d) => d > 25).length;
      message += `, p95 ${p95.toFixed(1)} ms, ${slow} slower than 25 ms`;
    }
    invoke("frontend_log", { level: "info", message }).catch(() => {});
    deltas = [];
    count = 0;
  }, WINDOW_MS);
  return () => {
    stop();
    clearInterval(timer);
  };
}
