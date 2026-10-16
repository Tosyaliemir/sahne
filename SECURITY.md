# Security policy

## Reporting a vulnerability

Please report vulnerabilities privately through GitHub's
[private vulnerability reporting](https://github.com/Tosyaliemir/sahne/security/advisories/new)
rather than in a public issue. You will receive an acknowledgement within a
week. Please include the Sahne version, your platform and steps to reproduce.

## Supported versions

Only the latest release receives security fixes.

## Threat model

Sahne is a local desktop application. It has no server component and no user
accounts. The relevant attack surface is the data it processes on the user's
behalf:

| Input                          | Source                                                                                                    | Trust        | Defences                                                                                                                                                                                                                                    |
| ------------------------------ | --------------------------------------------------------------------------------------------------------- | ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Track metadata and artwork URL | Any MPRIS client on the user's D-Bus session bus (Linux), or any app publishing a media session (Windows) | Untrusted    | See below                                                                                                                                                                                                                                   |
| Remote artwork                 | The URL above                                                                                             | Untrusted    | `https://` only; at most 3 redirects, each `https://`; hosts resolving to loopback, private, link-local, CGNAT or other non-public ranges are refused (also for literal IPs and redirect targets); bodies are streamed and capped at 15 MB  |
| Local artwork (`file://`)      | The URL above                                                                                             | Untrusted    | Only regular files up to 15 MB with a PNG, JPEG or WebP signature; the path is canonicalised and `/proc`, `/sys` and `/dev` are refused; a copy is cached under the app's cache directory, which is the only directory the webview may read |
| Image decoding                 | Remote or local artwork                                                                                   | Untrusted    | Decoded with limits (8192 × 8192 pixels, 128 MB allocation) against decompression bombs                                                                                                                                                     |
| Lyrics                         | LRCLIB (crowd-sourced)                                                                                    | Untrusted    | Responses are capped at 2 MB; the LRC parser uses checked/saturating arithmetic and is property-tested never to panic                                                                                                                       |
| Frontend log messages          | The app's own webview                                                                                     | Semi-trusted | Truncated to 4 KB, control characters escaped                                                                                                                                                                                               |

The webview runs under a strict Content Security Policy (no remote scripts,
images only from the app itself and its cache) and each window is granted only
the Tauri permissions it uses (`src-tauri/capabilities/`).

Out of scope: an attacker who can already run code as the user (they can read
the same files and talk to the same session bus directly).
