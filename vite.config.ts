import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import pkg from "./package.json" with { type: "json" };

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(({ command }) => ({
  // In dev, Vite 8 serves the raw .svelte source for the plugin's virtual
  // `?svelte&type=style` modules, so scoped styles break. Injecting CSS from JS
  // avoids those modules; production builds still extract a stylesheet.
  plugins: [svelte({ emitCss: command === "build" })],
  define: { __APP_VERSION__: JSON.stringify(pkg.version) },
  // Keep Rust compiler errors visible in `tauri dev`.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "es2022",
  },
  test: {
    include: ["src/**/*.test.ts"],
  },
}));
