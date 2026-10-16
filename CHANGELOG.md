# Changelog

All notable changes to Sahne are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Settings → About: version, licences of all bundled dependencies, third-party
  notices and disclaimers. The same files ship in the installers.
- The interface language follows the system locale by default (English or
  Turkish).

### Changed

- The bundle identifier is now `io.github.tosyaliemir.Sahne` (was
  `dev.sahne.app`). Settings and caches are unaffected; see "Migration".
- Settings store English values (`classic`, `fluid`, `electric`, …). Settings
  files from 0.1 still load.
- Descriptions and installer texts are in English and platform-neutral.

### Fixed

- Paused or hidden windows no longer draw frames: idle CPU use dropped from
  about 25 % to under 1 % of one core.

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

[Unreleased]: https://github.com/Tosyaliemir/sahne/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Tosyaliemir/sahne/releases/tag/v0.1.0
