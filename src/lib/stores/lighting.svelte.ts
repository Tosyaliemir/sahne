import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface LightingStatus {
  /** False when this build was compiled without the `lighting` feature. */
  available: boolean;
  state: "disabled" | "connecting" | "connected" | "offline";
  devices: { name: string; leds: number }[];
}

class LightingStore {
  status = $state.raw<LightingStatus>({ available: false, state: "disabled", devices: [] });

  async start(): Promise<() => void> {
    const unlisten = await listen<LightingStatus>("lighting-status", (e) => (this.status = e.payload));
    await this.refresh();
    return unlisten;
  }

  async refresh(): Promise<void> {
    try {
      this.status = await invoke<LightingStatus>("lighting_status");
    } catch (err) {
      console.warn("lighting_status failed", err);
    }
  }
}

export const lighting = new LightingStore();
