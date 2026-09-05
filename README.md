# Vinny

[![CI](https://github.com/sarimabbas/vinny/actions/workflows/ci.yml/badge.svg)](https://github.com/sarimabbas/vinny/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/sarimabbas/vinny)](https://github.com/sarimabbas/vinny/releases/latest)

Vinny is a small VNC server for macOS. It runs in the menu bar and lets you choose a display, capture size, frame rate, and whether viewers can control the Mac.

## Install

Requires macOS 12.3 or newer. The published download is for Apple silicon (ARM64). For other architectures, see the [build instructions](docs/CONTRIBUTING.md#development).

With Homebrew:

```bash
brew install --cask sarimabbas/tap/vinny
```

Or [download v0.2.4 for Apple silicon](https://github.com/sarimabbas/vinny/releases/download/v0.2.4/vinny-0.2.4-macos-arm64.zip), unzip it, and move `Vinny.app` to Applications. [Release notes and checksums](https://github.com/sarimabbas/vinny/releases/latest).

## Connect

Open Vinny and grant Screen Recording and Accessibility. Choose a display and enable the server. Both permissions are required for it to start.

The default listener is `127.0.0.1:5900`. To connect from another computer, follow the [connection guide](docs/first-connection.md).

[Settings reference](docs/share-selected-display.md) covers the server controls. The [Tailscale guide](docs/tailscale.md) covers private access through Serve. Closing the window leaves the servers running; quit Vinny to stop them.

## Connection security

Vinny defaults to an **unauthenticated loopback listener**. For access from another computer, use the [SSH tunnel](docs/first-connection.md), or configure encrypted connections with a compatible viewer.

**Encrypted + password** supports VeNCrypt/X509Plain, including TigerVNC. Vinny creates a self-signed certificate when a server starts. Its fingerprint changes when the server is recreated, so use an authenticated tunnel when you need a stable, verified endpoint.

The optional **Legacy authentication (unencrypted)** setting allows legacy VNC clients such as macOS Screen Sharing. It limits passwords to eight bytes and does **not** encrypt screen contents or input. Use it through a secure tunnel. View-only access still exposes screen and outgoing clipboard contents.

See the [threat model](docs/threat-model.md) and [security policy](docs/SECURITY.md).

## Implementation

Vinny captures displays with ScreenCaptureKit, serves them over standard RFB, and forwards keyboard and pointer events through macOS input APIs. Capture and input account for Retina scaling.

It supports RFB 3.3 through 3.8, common framebuffer encodings, framebuffer resizing, cursor metadata, extended key events, and clipboard sync. Each server accepts at most eight clients, with a 10-second handshake timeout.

- [`screencapturekit`](https://crates.io/crates/screencapturekit): MIT/Apache-2.0 capture bindings
- [`rustvncserver`](https://crates.io/crates/rustvncserver): Apache-2.0 RFB server, vendored at 2.2.1
- [`enigo`](https://crates.io/crates/enigo): MIT input injection

## Development and documentation

[Build, package, and smoke test](docs/CONTRIBUTING.md) · [Release and notarize](docs/RELEASING.md) · [All documentation](docs/README.md)

## License

[Apache-2.0](LICENSE). Vendored components retain their original notices and licenses.
