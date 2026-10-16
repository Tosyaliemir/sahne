// Prints the CHANGELOG.md section of a version as release notes:
//   node dev-tools/release/notes.mjs 0.2.0 > notes.md
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** The body of `## [version]` up to the next `## [` heading, or null. */
export function sectionOf(changelog, version) {
  const lines = changelog.split("\n");
  const start = lines.findIndex((l) => l.startsWith(`## [${version}]`));
  if (start < 0) return null;
  let end = lines.findIndex((l, i) => i > start && l.startsWith("## ["));
  if (end < 0) end = lines.findIndex((l, i) => i > start && /^\[[^\]]+\]: /.test(l));
  return lines
    .slice(start + 1, end < 0 ? undefined : end)
    .join("\n")
    .trim();
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const version = (process.argv[2] ?? "").replace(/^v/, "");
  const changelog = readFileSync(new URL("../../CHANGELOG.md", import.meta.url), "utf8");
  const body = sectionOf(changelog, version);
  if (!body) {
    console.error(`CHANGELOG.md has no section for ${version}`);
    process.exit(1);
  }
  process.stdout.write(`${body}

## Downloads

- **Linux:** \`.AppImage\` (any distribution: \`chmod +x\` and run) or \`.deb\` (Debian, Ubuntu).
- **Windows:** \`Sahne_${version}_x64-setup.exe\`. The installer is not code-signed yet; if SmartScreen
  warns, choose **More info → Run anyway**.

Verify a download with \`sha256sum -c SHA256SUMS --ignore-missing\` and its build provenance with
\`gh attestation verify <file> --repo Tosyaliemir/sahne\`. See the README for installation and usage.
`);
}
