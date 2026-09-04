# Vinny

[![CI](https://github.com/sarimabbas/vinny/actions/workflows/ci.yml/badge.svg)](https://github.com/sarimabbas/vinny/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/sarimabbas/vinny)](https://github.com/sarimabbas/vinny/releases/latest)

Vinny is a small VNC server for macOS. It captures configured displays with ScreenCaptureKit, serves them through standard RFB, and forwards keyboard and pointer events through macOS input APIs.

Vinny is a menu-bar app. Open it, grant Screen Recording and Accessibility once, then connect any VNC client. The default server listens only on `127.0.0.1:5900`.

## Install

Use the fully qualified cask name so Homebrew trusts only the Vinny cask:

```bash
brew install --cask sarimabbas/tap/vinny
```

Then open Vinny from Applications.

## Build

Requires macOS 12.3+, Xcode, and Rust. The repository pins Rust 1.90 because `rustvncserver` requires it.

```bash
cargo build
./scripts/package.sh
open dist/Vinny.app
```

The app asks for Screen Recording and Accessibility access. Enabled servers start after both permissions are granted. New servers use the next free port starting at `5900`.

Vinny appears in the Dock while its window is open. Closing the window hides the Dock icon but leaves the servers running.

## Package locally

For an ad-hoc signed development build:

```bash
./scripts/package.sh
```

For a Developer ID build:

```bash
SIGN_IDENTITY='Developer ID Application: …' ./scripts/package.sh
```

To sign, notarize, staple, and archive a release with the default `developer-notary` keychain profile:

```bash
SIGN_IDENTITY='Developer ID Application: …' ./scripts/release.sh
```

Set `NOTARY_PROFILE` to use a differently named keychain profile. The archive and its SHA-256 checksum are written to `dist/`.

Keep the Developer ID identity and `run.lil.vinny` bundle identifier stable. macOS ties privacy grants to that identity.

## Smoke test

After packaging and granting permissions:

```bash
./scripts/smoke.sh
```

The smoke launches `Vinny.app`, verifies that port 5900 is loopback-only, completes an RFB 3.8 handshake, requests a framebuffer, and confirms captured pixels are non-empty.

## Capabilities

Each server configuration selects a display, maximum width, frame rate, address, port, sharing policy, and whether remote control or encryption is enabled. Settings persist between launches.

Vinny supports RFB 3.3 through 3.8, common framebuffer encodings, framebuffer resizing, cursor metadata, extended key events, and clipboard sync. Capture and input account for Retina scaling. Handshakes time out after 10 seconds, and each server accepts at most eight clients.

> [!WARNING]
> Vinny defaults to an unauthenticated loopback listener for compatibility. Enable “Encrypted + password” before exposing a listener when your VNC viewer supports VeNCrypt/X509Plain. Otherwise use a trusted network or secure tunnel. View-only mode blocks remote input but still exposes screen contents.

TigerVNC is tested. Vinny does not work with macOS Screen Sharing because that client requests legacy VNC Authentication.

## Implementation

- [`screencapturekit`](https://crates.io/crates/screencapturekit): MIT/Apache-2.0 capture bindings
- [`rustvncserver`](https://crates.io/crates/rustvncserver): Apache-2.0 RFB server
- [`enigo`](https://crates.io/crates/enigo): MIT input injection

Vinny vendors `rustvncserver` 2.2.1 for exact-address binding, connection limits, RFB version negotiation, VeNCrypt/X.509 TLS, and the extensions listed above.

All resolved Rust dependencies use permissive licenses. None use the GPL or AGPL.

## Documentation

Development, release, security, and design documentation is indexed in [`docs/`](docs/README.md).

## License

[Apache-2.0](LICENSE). Vendored components retain their original notices and licenses.
