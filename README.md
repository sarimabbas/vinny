# Vinny

[![CI](https://github.com/sarimabbas/vinny/actions/workflows/ci.yml/badge.svg)](https://github.com/sarimabbas/vinny/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/sarimabbas/vinny)](https://github.com/sarimabbas/vinny/releases/latest)

Vinny is a small VNC server for macOS. It captures configured displays with ScreenCaptureKit, serves them through standard RFB, and forwards keyboard and pointer events through macOS input APIs.

Vinny is a menu-bar app. Open it, grant Screen Recording and Accessibility once, then connect any VNC client. The default server listens only on `127.0.0.1:5900`.

## Status

Early MVP:

- One configurable VNC server per display, all within one app process
- Persistent display, port, maximum-width, frame-rate, and listen-address settings
- Retina-aware input scaling
- Raw, Hextile, Tight, ZRLE, and other noVNC-compatible encodings through `rustvncserver`
- Mouse, scrolling, common keys, modifiers, and Unicode input
- Guided Screen Recording and Accessibility permission setup
- Configurable IPv4 or IPv6 listeners, with a safe loopback default

Not yet implemented: VNC authentication, clipboard sync, and login-window control.

> [!WARNING]
> The RFB server currently has no built-in authentication or encryption. Vinny defaults to loopback. Use another listen address only on a trusted network, and never expose Vinny directly to the public internet.

## Install

Homebrew 6 and newer require explicit trust for third-party taps:

```bash
brew tap sarimabbas/tap
brew trust --cask sarimabbas/tap/vinny
brew install --cask vinny
```

Then open Vinny from Applications.

## Build

Requires macOS 12.3+, Xcode, and Rust. The repository pins Rust 1.90 because `rustvncserver` requires it.

```bash
cargo build
./scripts/package.sh
open dist/Vinny.app
```

The app opens a guided permission window. After both permissions are granted, enabled servers start automatically. Add another server to share another display—ports advance from `5900` by default. Vinny appears in the Dock while its window is open; closing the window returns it to a menu-bar-only app without stopping any servers.

## Package locally

Create an ad-hoc signed app for local development:

```bash
./scripts/package.sh
```

Create a Developer ID signed app:

```bash
SIGN_IDENTITY='Developer ID Application: …' ./scripts/package.sh
```

To sign, notarize, staple, and create a versioned release archive using credentials saved under the default `developer-notary` keychain profile:

```bash
SIGN_IDENTITY='Developer ID Application: …' ./scripts/release.sh
```

Set `NOTARY_PROFILE` to use a differently named keychain profile. The archive and its SHA-256 checksum are written to `dist/`.

A stable Developer ID signature and the `run.lil.vinny` bundle identifier are important because macOS associates privacy grants with code identity.

## Smoke test

After packaging and granting permissions:

```bash
./scripts/smoke.sh
```

The smoke launches `Vinny.app`, verifies that port 5900 is loopback-only, completes an RFB 3.8 handshake, requests a framebuffer, and confirms captured pixels are non-empty.

## Implementation

- [`screencapturekit`](https://crates.io/crates/screencapturekit) — MIT/Apache-2.0 capture bindings
- [`rustvncserver`](https://crates.io/crates/rustvncserver) — Apache-2.0 RFB server
- [`enigo`](https://crates.io/crates/enigo) — MIT input injection

`rustvncserver` 2.2.1 binds all interfaces in its public `listen(port)` method. A vendored one-method patch adds `listen_on(SocketAddr)` so Vinny can bind to the exact address configured in the app.

All resolved Rust dependencies are permissively licensed; there are no GPL or AGPL dependencies.

## Documentation

Development, release, security, and design documentation is indexed in [`docs/`](docs/README.md).

## License

[Apache-2.0](LICENSE). Vendored components retain their original notices and licenses.
