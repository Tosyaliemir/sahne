export interface ReflectiveOptions {
  theme?: "dark" | "light";
  tint?: string;
  metalness?: number;
  roughness?: number;
  blurStrength?: number;
  grayscale?: number;
  warp?: number;
  warpScale?: number;
  shine?: number;
  bevel?: number;
  glare?: number;
  enableTilt?: boolean;
  tiltStrength?: number;
  radius?: number;
  intro?: boolean;
}
export function reflectiveSettings(options?: ReflectiveOptions): Record<string, unknown>;
export function createReflectivePlate(
  els: {
    root: HTMLElement;
    body: HTMLElement;
    canvas: HTMLCanvasElement;
    image: HTMLImageElement;
  },
  getSettings: () => ReturnType<typeof reflectiveSettings>,
  opts?: { isFirst?: boolean; onRestore?: () => void },
): { wake: () => void; destroy: () => void };
