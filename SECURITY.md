# Security policy

## Reporting a vulnerability

Please report vulnerabilities privately through GitHub's
[private vulnerability reporting](https://github.com/Tosyaliemir/sahne/security/advisories/new),
not in a public issue. Include the Sahne version (Settings → About), your
platform, and steps to reproduce or a proof of concept.

You can expect an acknowledgement within one week and an assessment within two
weeks. Fixes are released as soon as practical, and the advisory is published
once a fixed release is available. Reporters are credited unless they prefer
otherwise.

## Supported versions

| Version        | Supported |
| -------------- | --------- |
| Latest release | Yes       |
| Older releases | No        |

## Threat model

Sahne is a local desktop application with no server component and no user
accounts. Its attack surface is the data it processes on the user's behalf.

**Track metadata and the artwork URL** come from any MPRIS client on the
user's D-Bus session bus (Linux) or any application that publishes a media
session (Windows). They are untrusted.

**Remote artwork** (from that URL) is untrusted:

- only `https://` is allowed, with at most three redirects, each `https://`;
- hosts that resolve to loopback, private, link-local, CGNAT or other
  non-public ranges are refused, including literal IP addresses and redirect
  targets;
- bodies are streamed and capped at 15 MB.

**Local artwork** (`file://` URLs) is untrusted:

- only regular files of up to 15 MB with a PNG, JPEG or WebP signature are
  read;
- the path is canonicalised, and `/proc`, `/sys` and `/dev` are refused;
- a copy is cached in the app's cache directory, the only directory the
  webview may read.

**Image decoding** runs with limits (8192 × 8192 pixels, 128 MB of
allocations) against decompression bombs.

**Lyrics** come from LRCLIB, a crowd-sourced service, and are untrusted.
Responses are capped at 2 MB, and the LRC parser uses checked arithmetic and is
property-tested never to panic.

**Frontend log messages** come from the app's own webview and are
semi-trusted. They are truncated to 4 KB and control characters are escaped.

The webview runs under a strict Content Security Policy (no remote scripts;
images only from the app itself and its cache), and each window is granted
only the Tauri permissions it uses (`src-tauri/capabilities/`). Dependencies
are checked in CI with `cargo deny` and `pnpm audit`.

**Out of scope:** an attacker who can already run code as the user, since they
can read the same files and talk to the same session bus directly.
