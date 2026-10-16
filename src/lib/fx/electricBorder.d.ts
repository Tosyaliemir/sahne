export const MARGIN: number;
export interface ElectricOptions {
  color?: string;
  glowColor?: string;
  speed?: number;
  chaos?: number;
  spread?: number;
  thickness?: number;
  strands?: number;
  glow?: number;
  intensity?: number;
  arcs?: number;
  flicker?: number;
  borderRadius?: number;
  offset?: number;
  active?: boolean;
  intro?: boolean;
  interactive?: boolean;
  cursorRadius?: number;
  theme?: "dark" | "light";
  /** Freeze on the current frame and stop rendering (e.g. while paused). */
  paused?: boolean;
}
export function electricSettings(options?: ElectricOptions): Record<string, unknown>;
export function createElectricBorder(
  container: HTMLElement,
  canvas: HTMLCanvasElement,
  getSettings: () => ReturnType<typeof electricSettings>,
  opts?: { isFirst?: boolean; onRestore?: () => void },
): { refresh: () => void; destroy: () => void } | null;
