# Contributing to Sahne

Thank you for your interest in improving Sahne. Bug reports, translations,
documentation and code are all welcome. By taking part you agree to follow the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Reporting problems

- **Bugs and feature requests:** open an issue using one of the forms.
- **Security vulnerabilities:** report them privately as described in
  [SECURITY.md](SECURITY.md), never in a public issue.
- **Questions:** use GitHub Discussions.

## Development setup

You need Rust (the version in `rust-toolchain.toml` is installed
automatically by `rustup`), Node.js 22 and pnpm 10.

**Linux system packages**

```sh
# Debian / Ubuntu
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev
# Fedora
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel \
  librsvg2-devel libxdo-devel && sudo dnf group install "c-development"
# Arch
sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module \
  libappindicator-gtk3 librsvg xdotool
```

**Windows:** install the Visual Studio C++ Build Tools and WebView2 (preinstalled
on Windows 11).

Then:

```sh
pnpm install
pnpm tauri dev
```

Useful environment variables while developing:

| Variable                     | Effect                                                     |
| ---------------------------- | ---------------------------------------------------------- |
| `RUST_LOG=sahne_lib=debug`   | Verbose log, including frontend errors (target `frontend`) |
| `SAHNE_PLAYER=sahnemock`     | Follow the mock player instead of Spotify                  |
| `SAHNE_SCREENSAVER_SECS=15`  | Trigger the screensaver after 15 s instead of minutes      |
| `SAHNE_TITLE="Sahne (test)"` | Give a test instance a different window title              |

In development builds the frontend logs `frames: N frames in 10 s` every ten
seconds; an idle or paused window should log `0 frames`.

### Testing without Spotify

`dev-tools/mock-player` publishes a fake MPRIS player on the session bus:

```sh
cd dev-tools/mock-player
cargo run                                  # three built-in tracks
cargo run -- --paused --zero-position      # start paused; report position 0 like some broken players
cargo run -- --track "Artist|Title|Album|213.5|file:///path/cover.png"
```

Type `p` (play/pause), `n` (next), `b` (previous), `s 60` (seek to 60 s) or
`q` (quit) in its terminal. Run Sahne with `SAHNE_PLAYER=sahnemock`.

### Previewing the UI in a browser

`preview.html` renders the UI against a mocked backend:

```sh
pnpm vite --port 1430
# http://127.0.0.1:1430/preview.html?view=main&style=kinetic&at=13000
```

The parameters are documented at the top of `dev-tools/preview/preview.ts`.

## Checks

Run these before opening a pull request; CI runs the same and more:

```sh
pnpm lint && pnpm check && pnpm test
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

`pnpm format` applies Prettier. Rust code is formatted with `cargo fmt`.

## Manual test checklist

Automated tests cannot cover a real desktop session. Before a release, check
these on as many of GNOME Wayland, KDE Plasma Wayland, X11 and Windows 11 as
you can, and note which ones were not tested:

- [ ] Change the track in Spotify: title, cover, colors and lyrics follow.
- [ ] Seek in Spotify: the lyrics jump to the new position at once.
- [ ] Pause and resume: the "Paused" badge appears and disappears, and the
      lyrics stop and continue. While paused, the dev build logs `0 frames`.
- [ ] Minimise or hide the window while playing: the dev build logs `0 frames`.
- [ ] Quit Spotify: the "Spotify isn't open" screen appears; start it again and Sahne
      reconnects by itself.
- [ ] Disconnect from the network and play a song not played before: the
      "Offline" screen appears, and the lyrics load once the network is back.
- [ ] Play a song without lyrics and an instrumental: large cover with "No
      lyrics found for this song" / "Instrumental".
- [ ] Play a Turkish song: ç, ğ, ı, İ, ö, ş, ü render correctly, including in
      the Wall style's capitals.
- [ ] Shift the lyrics with `[` / `]`, restart Sahne: the offset is kept.
- [ ] `L` switches between the split view and lyrics focus; `S` cycles through
      all six styles; `F` toggles fullscreen; `M` opens the mini player and
      back.
- [ ] `↑` / `↓` change the volume (Linux); the volume control is hidden on
      Windows.
- [ ] Settings → About opens the licence texts; the language follows the
      system when set to "System default".
- [ ] Screensaver: with `SAHNE_SCREENSAVER_SECS=15`, Sahne comes forward after
      15 s of inactivity while music plays and restores the windows on input.

## Commit style

Commits follow [Conventional Commits](https://www.conventionalcommits.org/):
`type(scope): summary` in the imperative mood, e.g.
`fix(lyrics): keep the offset when the track restarts`. Common types are
`feat`, `fix`, `perf`, `refactor`, `test`, `docs`, `build`, `ci` and `chore`.
Keep each commit focused and explain the reason in the body when it is not
obvious. User-facing changes also get a line under "Unreleased" in
[CHANGELOG.md](CHANGELOG.md).

## Adding a translation

1. Copy `src/lib/i18n/en.json` (the source of truth) to `src/lib/i18n/<code>.json`
   and translate the values. Keep every key and every `{placeholder}`.
2. Register the dictionary in `src/lib/i18n/index.svelte.ts` and teach
   `resolveLang` the locale prefix.
3. Add the language to the `Language` enum in `src-tauri/src/settings.rs` and
   to the language menu in `SettingsPanel.svelte`.
4. `pnpm test` checks that all dictionaries have the same keys and placeholders.

## Content rules

Do not add song lyrics or other copyrighted text to code, tests, fixtures or
screenshots. Use invented text instead. Screenshot fixtures stay in the
git-ignored `dev-tools/preview/fixtures/` directory.

## Releasing

See [docs/RELEASING.md](docs/RELEASING.md).

## Licensing

Contributions are accepted under the MIT License, the project's licence. Files
derived from third-party code keep their own licence and must be listed in
`THIRD_PARTY_NOTICES.md` and `REUSE.toml`; `reuse lint` runs in CI.
