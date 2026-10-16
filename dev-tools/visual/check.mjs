// Visual regression check (T2): renders fixed states of the preview harness
// with its invented lyrics and compares them with the baselines in
// dev-tools/visual/baseline/.
//   pnpm visual            compare; diffs go to dev-tools/visual/out/
//   pnpm visual --update   accept the current rendering as the baseline
// The states avoid anything random or time-based: playback is paused, CSS
// animations are off, and the WebGL backdrop and cover effects are not used.
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import pixelmatch from "pixelmatch";
import { chromium } from "playwright";
import { PNG } from "pngjs";
import { createServer } from "vite";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "../..");
const baselineDir = join(here, "baseline");
const outDir = join(here, "out");
const update = process.argv.includes("--update");
const PORT = 1432;
/** Share of pixels that may differ (anti-aliasing, font rasterisation). */
const MAX_DIFF_RATIO = 0.005;

const common = { lang: "en", static: "1", paused: "1", bg: "color", bar: "1" };
const cases = [
  { name: "classic", params: { style: "classic", at: "13500" } },
  { name: "classic-focus", params: { style: "classic", at: "13500", focus: "1" } },
  { name: "kinetic", params: { style: "kinetic", at: "13500", focus: "1" } },
  { name: "wall", params: { style: "wall", at: "19500", focus: "1" } },
  { name: "interlude", params: { style: "classic", at: "25000", focus: "1" } },
  { name: "plain", params: { lyrics: "plain" } },
  { name: "not-found", params: { lyrics: "not_found" } },
  { name: "settings", params: { panel: "settings", at: "13500" } },
  { name: "mini", params: { view: "mini", at: "13500" }, size: [500, 168] },
];

const server = await createServer({ root, logLevel: "warn", server: { port: PORT, strictPort: true } });
await server.listen();
const browser = await chromium.launch();
rmSync(outDir, { recursive: true, force: true });
mkdirSync(baselineDir, { recursive: true });
const failures = [];
try {
  for (const c of cases) {
    const [width, height] = c.size ?? [1280, 800];
    const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 });
    await page.goto(
      `http://127.0.0.1:${PORT}/preview.html?${new URLSearchParams({ ...common, ...c.params })}`,
    );
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(1200);
    const shot = PNG.sync.read(await page.screenshot());
    await page.close();
    const file = join(baselineDir, `${c.name}.png`);
    if (update || !existsSync(file)) {
      writeFileSync(file, PNG.sync.write(shot));
      console.log(`${c.name}: baseline written`);
      continue;
    }
    const base = PNG.sync.read(readFileSync(file));
    if (base.width !== shot.width || base.height !== shot.height) {
      failures.push(`${c.name}: size ${shot.width}x${shot.height}, baseline ${base.width}x${base.height}`);
      continue;
    }
    const diff = new PNG({ width: base.width, height: base.height });
    const changed = pixelmatch(base.data, shot.data, diff.data, base.width, base.height, { threshold: 0.1 });
    const ratio = changed / (base.width * base.height);
    console.log(`${c.name}: ${(ratio * 100).toFixed(3)} % of pixels differ`);
    if (ratio > MAX_DIFF_RATIO) {
      mkdirSync(outDir, { recursive: true });
      writeFileSync(join(outDir, `${c.name}-actual.png`), PNG.sync.write(shot));
      writeFileSync(join(outDir, `${c.name}-diff.png`), PNG.sync.write(diff));
      failures.push(`${c.name}: ${(ratio * 100).toFixed(2)} % differ (limit ${MAX_DIFF_RATIO * 100} %)`);
    }
  }
} finally {
  await browser.close();
  await server.close();
}
if (failures.length) {
  console.error(`\nVisual differences:\n  ${failures.join("\n  ")}\nSee dev-tools/visual/out/.`);
  process.exit(1);
}
