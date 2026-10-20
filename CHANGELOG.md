# Changelog

All notable changes to Sahne are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- The Linux backend uses zbus 6 for D-Bus (MPRIS, idle time, screensaver
  inhibition). zbus 5's helper crates are unmaintained (RUSTSEC-2026-0370,
  -0371, -0372).

## [0.2.0] - 2026-10-16

The 0.2.0 revision: security hardening, correctness and performance fixes,
licensing, an English-first repository, tests and release tooling.

### Security

- Artwork URLs from media players are treated as untrusted: `https://` only,
  no redirects to non-public addresses, bodies streamed and capped at 15 MB,
  decoding with pixel and memory limits, and `file://` artwork limited to real
  image files outside `/proc`, `/sys` and `/dev`.
- LRCLIB responses are capped at 2 MB; the LRC parser can no longer overflow on
  hostile timestamps or offsets.
- A strict Content Security Policy and per-window minimal Tauri permissions.
- Frontend log messages are size-limited and escaped; the temporary-directory
  fallback is private, and file writes are atomic and crash-safe.
- Unused camera and video code removed from the metal plate effect.
- GitHub Actions pinned to commit SHAs with least-privilege permissions;
  dependencies checked with `cargo deny` and `pnpm audit`. See SECURITY.md.

### Added

- Settings → About: version, licences of all bundled dependencies, third-party
  notices and disclaimers. The same files ship in the installers.
- The interface language follows the system locale by default (English or
  Turkish).
- Performance mode (Auto, Quality, Battery saver) for the animated backdrop and
  the cover effects.
- A Clear button for the caches, which are now size-capped and pruned at
  startup.
- Accessibility: the settings panel is a proper modal dialog, every control
  shows keyboard focus, the current lyric lines can be focused and Enter seeks
  to them, and an optional setting announces new lines to screen readers.
- Cover effects follow the system's "more contrast" preference.

### Changed

- The bundle identifier is now `io.github.tosyaliemir.Sahne` (was
  `dev.sahne.app`). Settings and caches are unaffected; see "Migration".
- Settings store English values (`classic`, `fluid`, `electric`, …). Settings
  files from 0.1 still load.
- Descriptions and installer texts are in English and platform-neutral; the
  README is in English with a Turkish translation.
- The settings file is read leniently: one invalid value resets only that
  setting, and an unreadable file is backed up before it is replaced.

### Fixed

- Paused or hidden windows no longer draw frames: idle CPU use dropped from
  about 25 % to under 1 % of one core.
- Changes made in one window (or by the backend) are no longer lost when the
  other window saves settings; settings and offsets are saved off the main
  thread.
- Skipping tracks quickly no longer piles up artwork downloads, and a player
  reusing one artwork file now gets the new cover.
- Turning the screensaver off while it shows restores the windows at once.
- The Wall style shows the interlude indicator during instrumental breaks.
- A stopped player shows "Stopped" instead of "Paused".
- A player at the very start of a track is no longer briefly mistaken for one
  that is stuck at position 0.
- Windows: stable artwork cache names and correct paths containing `%`; the
  media-session loop recovers from errors instead of crashing.
- A crash now leaves a report (`crash.log`) in the cache directory.

### Migration

- The old identifier's cache directory (`~/.cache/dev.sahne.app` on Linux,
  `%LOCALAPPDATA%\dev.sahne.app` on Windows) is removed on first start. Its
  WebView data directory (`~/.local/share/dev.sahne.app`,
  `%LOCALAPPDATA%\dev.sahne.app\EBWebView`) holds nothing Sahne needs and can
  be deleted by hand.

## [0.1.0] - 2026-10-15

### Added

- First release for Linux (AppImage, .deb) and Windows (installer).
- Synchronized, word-by-word animated lyrics from LRCLIB for the track playing
  in Spotify or any MPRIS / system media session player.
- Lyrics styles: Classic, Kinetic, Minimal, Wave, Blend and Wall.
- Backdrops made from the album artwork: fluid (WebGL), blurred, cover and
  color; optional edge glow and cover effects (metal plate, electric border).
- Mini player, screensaver mode, per-track lyric offsets and an on-disk cache.

[Unreleased]: https://github.com/Tosyaliemir/sahne/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/Tosyaliemir/sahne/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Tosyaliemir/sahne/releases/tag/v0.1.0
