// Prepares a release:  pnpm release 0.2.0
// Sets the version in package.json, Cargo.toml and Cargo.lock (tauri.conf.json
// reads package.json), turns "Unreleased" in CHANGELOG.md into the new
// version, regenerates the licence lists, commits, and creates an annotated
// tag. Nothing is pushed; push the commit and the tag yourself to start the
// release workflow.
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
const run = (cmd, args, opts = {}) =>
  execFileSync(cmd, args, { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"], ...opts });

/** Moves the Unreleased section under a new version heading. */
export function releaseChangelog(text, version, date) {
  if (!/^## \[Unreleased\]$/m.test(text)) throw new Error('CHANGELOG.md has no "## [Unreleased]" heading');
  if (text.includes(`## [${version}]`)) throw new Error(`CHANGELOG.md already has ${version}`);
  const previous = /^## \[(\d+\.\d+\.\d+[^\]]*)\]/m.exec(text.replace(/^## \[Unreleased\]$/m, ""))?.[1];
  let out = text.replace(/^## \[Unreleased\]$/m, `## [Unreleased]\n\n## [${version}] - ${date}`);
  const repo = "https://github.com/Tosyaliemir/sahne";
  out = out.replace(/^\[Unreleased\]: .*$/m, `[Unreleased]: ${repo}/compare/v${version}...HEAD`);
  const link = previous
    ? `[${version}]: ${repo}/compare/v${previous}...v${version}`
    : `[${version}]: ${repo}/releases/tag/v${version}`;
  return out.replace(/^(\[Unreleased\]: .*)$/m, `$1\n${link}`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const version = (process.argv[2] ?? "").replace(/^v/, "");
  if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version)) {
    console.error("usage: pnpm release <major.minor.patch>");
    process.exit(2);
  }
  if (run("git", ["status", "--porcelain"]).trim()) {
    console.error("The working tree has changes; commit or stash them first.");
    process.exit(1);
  }
  const date = new Date().toISOString().slice(0, 10);

  const pkgPath = `${root}/package.json`;
  const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
  pkg.version = version;
  writeFileSync(pkgPath, `${JSON.stringify(pkg, null, 2)}\n`);

  const cargoPath = `${root}/src-tauri/Cargo.toml`;
  const cargo = readFileSync(cargoPath, "utf8");
  writeFileSync(cargoPath, cargo.replace(/^version = ".*"$/m, `version = "${version}"`));
  run("cargo", ["update", "--workspace", "--offline"], { cwd: `${root}/src-tauri` });

  const changelogPath = `${root}/CHANGELOG.md`;
  writeFileSync(changelogPath, releaseChangelog(readFileSync(changelogPath, "utf8"), version, date));

  // The Rust licence list names Sahne's own version; CI fails if it is stale.
  run("node", ["dev-tools/licenses/generate.mjs"], { stdio: ["ignore", "ignore", "inherit"] });

  run("git", [
    "add",
    "package.json",
    "src-tauri/Cargo.toml",
    "src-tauri/Cargo.lock",
    "CHANGELOG.md",
    "src/assets/licenses",
  ]);
  run("git", ["commit", "-m", `chore(release): ${version}`]);
  run("git", ["tag", "-a", `v${version}`, "-m", `Sahne ${version}`]);
  console.log(`Tagged v${version}. Push it with:\n  git push && git push origin v${version}`);
}
