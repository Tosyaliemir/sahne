// Renders docs/social-preview.png (1280x640), the image for GitHub's social
// preview setting, from the preview harness's invented content (no real
// lyrics or artwork):  node dev-tools/screenshots/social.mjs
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";

const root = fileURLToPath(new URL("../..", import.meta.url));
const PORT = 1434;
const server = await createServer({ root, logLevel: "warn", server: { port: PORT, strictPort: true } });
await server.listen();
const browser = await chromium.launch({ args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 640 }, deviceScaleFactor: 1 });
  const query = new URLSearchParams({
    lang: "en",
    static: "1",
    perf: "quality",
    at: "12300",
    style: "classic",
  });
  await page.goto(`http://127.0.0.1:${PORT}/preview.html?${query}`);
  await page.waitForTimeout(2800);
  await page.screenshot({ path: join(root, "docs/social-preview.png") });
} finally {
  await browser.close();
  await server.close();
}
