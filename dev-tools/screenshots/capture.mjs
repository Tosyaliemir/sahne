// Captures the README screenshots from the preview harness:
//   pnpm preview:fixture am-i-dreaming   (once, while the track plays)
//   pnpm screenshots
// Shots are rendered at 2x in headless Chromium, downscaled, and written as
// WebP (at most 300 KB each) to docs/screenshots/. Only the images are
// committed; the fixture with the lyrics stays in the git-ignored
// dev-tools/preview/fixtures/.
import { mkdirSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import sharp from "sharp";
import { createServer } from "vite";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const out = join(root, "docs/screenshots");
const FIXTURE = process.env.FIXTURE ?? "am-i-dreaming";
const MAX_BYTES = 300 * 1024;
const PORT = 1431;

/** Positions are where the shot lands; the page starts `settle` ms earlier. */
const shots = [
  { file: "hero", at: 28_000, params: { style: "classic", bar: "1" } },
  { file: "focus-classic", at: 152_000, params: { style: "classic", focus: "1" } },
  { file: "kinetic", at: 76_150, params: { style: "kinetic", focus: "1" } },
  { file: "wave", at: 84_000, params: { style: "wave", focus: "1" } },
  { file: "wall", at: 102_800, params: { style: "wall", focus: "1" } },
  { file: "blurred", at: 167_500, params: { style: "classic", bg: "blurred" } },
  { file: "cover-metal", at: 119_000, params: { style: "classic", cover: "metal" } },
  { file: "cover-electric", at: 12_000, params: { style: "classic", cover: "electric" } },
  { file: "mini", at: 177_500, params: { view: "mini" }, size: [500, 168] },
  { file: "settings", at: 209_500, params: { style: "classic", panel: "settings" } },
];

const SIZE = [1600, 1000];
const SETTLE_MS = 2800; // past the 2 s auto-hide of the bottom bar

async function encode(png, [width, height], file) {
  for (let quality = 82; quality >= 50; quality -= 6) {
    await sharp(png).resize(width, height, { kernel: "lanczos3" }).webp({ quality, effort: 6 }).toFile(file);
    if (statSync(file).size <= MAX_BYTES) return quality;
  }
  throw new Error(`${file} stays above ${MAX_BYTES} bytes`);
}

const server = await createServer({ root, logLevel: "warn", server: { port: PORT, strictPort: true } });
await server.listen();
const browser = await chromium.launch({
  args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"],
});
mkdirSync(out, { recursive: true });
try {
  for (const shot of shots) {
    const size = shot.size ?? SIZE;
    const page = await browser.newPage({
      viewport: { width: size[0], height: size[1] },
      deviceScaleFactor: 2,
    });
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    const query = new URLSearchParams({
      fixture: FIXTURE,
      lang: "en",
      static: "1",
      perf: "quality",
      at: String(shot.at - SETTLE_MS),
      ...shot.params,
    });
    await page.goto(`http://127.0.0.1:${PORT}/preview.html?${query}`);
    await page.waitForTimeout(SETTLE_MS);
    const png = await page.screenshot();
    const file = join(out, `${shot.file}.webp`);
    const quality = await encode(png, size, file);
    console.log(`${shot.file}.webp  q${quality}  ${(statSync(file).size / 1024).toFixed(0)} KB`);
    if (errors.length) console.warn(`  page errors: ${errors.join("; ")}`);
    await page.close();
  }
} finally {
  await browser.close();
  await server.close();
}
