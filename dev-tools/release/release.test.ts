import { describe, expect, it } from "vitest";
// @ts-expect-error plain JavaScript dev tool
import { sectionOf } from "./notes.mjs";
// @ts-expect-error plain JavaScript dev tool
import { releaseChangelog } from "./release.mjs";

const changelog = `# Changelog

## [Unreleased]

### Fixed

- Something.

## [0.1.0] - 2026-10-15

### Added

- First release.

[Unreleased]: https://github.com/Tosyaliemir/sahne/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Tosyaliemir/sahne/releases/tag/v0.1.0
`;

describe("release tooling (CI2, CI4)", () => {
  it("moves Unreleased under the new version and links it", () => {
    const out = releaseChangelog(changelog, "0.2.0", "2026-11-01");
    expect(out).toContain("## [Unreleased]\n\n## [0.2.0] - 2026-11-01\n\n### Fixed");
    expect(out).toContain("[Unreleased]: https://github.com/Tosyaliemir/sahne/compare/v0.2.0...HEAD");
    expect(out).toContain("[0.2.0]: https://github.com/Tosyaliemir/sahne/compare/v0.1.0...v0.2.0");
    expect(() => releaseChangelog(out, "0.2.0", "2026-11-01")).toThrow();
  });

  it("extracts one version's notes", () => {
    const out = releaseChangelog(changelog, "0.2.0", "2026-11-01");
    expect(sectionOf(out, "0.2.0")).toBe("### Fixed\n\n- Something.");
    expect(sectionOf(out, "0.1.0")).toBe("### Added\n\n- First release.");
    expect(sectionOf(out, "9.9.9")).toBeNull();
  });
});
