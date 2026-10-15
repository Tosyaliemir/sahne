import { invoke } from "@tauri-apps/api/core";

/**
 * Development aid: logs frame pacing every 10 s (average fps, 95th-percentile
 * frame time, frames slower than 25 ms) into the backend log.
 */
export function startFrameStats(label: string): () => void {
  const WINDOW_MS = 10_000;
  let deltas: number[] = [];
  let last = 0;
  let windowStart = 0;
  let raf = requestAnimationFrame(function loop(now) {
    if (last) deltas.push(now - last);
    last = now;
    windowStart ||= now;
    if (now - windowStart >= WINDOW_MS && deltas.length > 0) {
      const sorted = [...deltas].sort((a, b) => a - b);
      const avg = deltas.reduce((a, b) => a + b, 0) / deltas.length;
      const p95 = sorted[Math.floor(sorted.length * 0.95)];
      const slow = deltas.filter((d) => d > 25).length;
      const message = `${label}: ${(1000 / avg).toFixed(1)} fps, p95 ${p95.toFixed(1)} ms, ${slow}/${deltas.length} frames > 25 ms`;
      invoke("frontend_log", { level: "info", message }).catch(() => {});
      deltas = [];
      windowStart = now;
    }
    raf = requestAnimationFrame(loop);
  });
  return () => cancelAnimationFrame(raf);
}
