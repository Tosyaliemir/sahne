# Releasing

## Making a release

1. Make sure `main` is green in CI and `CHANGELOG.md` lists every user-facing
   change under "Unreleased".
2. Go through the [manual test checklist](../CONTRIBUTING.md#manual-test-checklist)
   on the platforms you have.
3. Run `pnpm release <version>` (for example `pnpm release 0.2.0`). It sets the
   version in `package.json`, `src-tauri/Cargo.toml` and `Cargo.lock`
   (`tauri.conf.json` reads `package.json`), moves "Unreleased" in the
   changelog under the new version, regenerates the licence lists (this needs
   `cargo-about`), commits, and creates an annotated tag.
   Nothing is pushed.
4. Push: `git push && git push origin v<version>`.
5. The **Release** workflow builds the Linux packages (x86_64, plus aarch64 when
   that runner is available) and the Windows installer, writes `SHA256SUMS`,
   attests build provenance for every file, and attaches everything to a
   **draft** release whose notes come from the changelog.
6. Review the draft, download and try a package, then publish it.

Users can check a download with:

```sh
sha256sum -c SHA256SUMS --ignore-missing
gh attestation verify Sahne_<version>_amd64.AppImage --repo Tosyaliemir/sahne
```

## Prepared but disabled

These are wired up but stay off until someone sets them up.

### In-app updates (Tauri updater)

The release workflow can produce signed update artifacts and a `latest.json`
manifest. To turn it on:

1. Generate a key pair: `pnpm tauri signer generate -w ~/.tauri/sahne.key`.
   Keep the private key safe; losing it means users can no longer update.
2. Add the repository secrets `TAURI_SIGNING_PRIVATE_KEY` (the private key's
   content) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
3. Put the public key's content into `src-tauri/tauri.updater.conf.json`.
4. Add the updater to the app: the `tauri-plugin-updater` crate, its
   registration in `lib.rs`, the `updater:default` permission for the main
   window, and a "check for updates" action in Settings → About.
5. Set the repository variable `ENABLE_UPDATER` to `true`.

Until then, releases contain no `.sig` files or `latest.json`.

### Flatpak

`packaging/flatpak/io.github.tosyaliemir.Sahne.yml` repackages a release's
`.deb`, with `packaging/linux/io.github.tosyaliemir.Sahne.metainfo.xml` as its
AppStream data. To build it, set the release URL and `sha256` in the manifest
and run:

```sh
flatpak-builder --user --install build packaging/flatpak/io.github.tosyaliemir.Sahne.yml
```

The sandbox gets network access, the MPRIS bus names and the idle and
screensaver interfaces it needs (see the manifest). It has not been tested yet.
Publishing on Flathub needs a from-source build with vendored cargo and npm
sources (flatpak-builder-tools) instead of the `.deb`.

### Windows code signing

The installer is unsigned, so SmartScreen warns on first run. To sign it, set
the repository variable `WINDOWS_SIGN_COMMAND` to a command that signs the file
given as `%1` (for example Azure Trusted Signing through
`trusted-signing-cli`), and provide its credentials as secrets referenced by
that command's environment. The release workflow then passes it to Tauri as
`bundle.windows.signCommand`.
