// Builds a screenshot fixture from the track playing right now:
//   pnpm preview:fixture [name] [--player spotify]
// Metadata comes from the player over MPRIS (playerctl); lyrics and artwork
// come from Sahne's cache, processed by the app's own Rust code. The output
// goes to dev-tools/preview/fixtures/, which is git-ignored: lyrics must
// never be committed.
import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const out = join(root, "dev-tools/preview/fixtures");
const args = process.argv.slice(2);
const playerAt = args.indexOf("--player");
const player = playerAt >= 0 ? args[playerAt + 1] : "spotify";
const positional = args.filter((_, i) => i !== playerAt && i !== playerAt + 1);

const field = (key) =>
  execFileSync("playerctl", ["-p", player, "metadata", key], { encoding: "utf8" }).trim();
const meta = {
  id: field("mpris:trackid"),
  title: field("xesam:title"),
  artist: field("xesam:artist"),
  album: field("xesam:album"),
  lengthMs: String(Math.floor(Number(field("mpris:length")) / 1000)),
  art: field("mpris:artUrl"),
};
const name =
  positional[0] ??
  meta.title
    .toLowerCase()
    .replace(/\(.*?\)/g, "")
    .trim()
    .replace(/[^a-z0-9]+/g, "-");

mkdirSync(out, { recursive: true });
execFileSync(
  "cargo",
  [
    "run",
    "--quiet",
    "--features",
    "dev-fixture",
    "--example",
    "preview_fixture",
    "--",
    out,
    name,
    meta.id,
    meta.title,
    meta.artist,
    meta.album,
    meta.lengthMs,
    meta.art,
  ],
  { cwd: join(root, "src-tauri"), stdio: "inherit" },
);
console.log(`fixture "${name}": ${meta.artist} – ${meta.title}`);
console.log(`open http://127.0.0.1:1430/preview.html?fixture=${name}`);
