// Writes the Tauri updater manifest (latest.json) for a release from the
// signed packages in a directory. Used by the release workflow only when the
// updater is enabled (see docs/RELEASING.md):
//   node dev-tools/release/latest-json.mjs <version> <packages-dir> > latest.json
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { sectionOf } from "./notes.mjs";

const [version, dir] = process.argv.slice(2);
const base = `https://github.com/Tosyaliemir/sahne/releases/download/v${version}`;
const targets = [
  ["linux-x86_64", /_amd64\.AppImage$/],
  ["linux-aarch64", /_aarch64\.AppImage$/],
  ["windows-x86_64", /_x64-setup\.exe$/],
];
const files = readdirSync(dir);
const platforms = {};
for (const [platform, pattern] of targets) {
  const file = files.find((f) => pattern.test(f));
  if (!file || !files.includes(`${file}.sig`)) continue;
  platforms[platform] = {
    signature: readFileSync(join(dir, `${file}.sig`), "utf8").trim(),
    url: `${base}/${encodeURIComponent(file)}`,
  };
}
if (Object.keys(platforms).length === 0) {
  console.error(`no signed packages in ${dir}`);
  process.exit(1);
}
const changelog = readFileSync(new URL("../../CHANGELOG.md", import.meta.url), "utf8");
const manifest = {
  version,
  notes: sectionOf(changelog, version) ?? "",
  pub_date: new Date().toISOString(),
  platforms,
};
process.stdout.write(`${JSON.stringify(manifest, null, 2)}\n`);
