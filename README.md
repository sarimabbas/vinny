# macos-vnc-server

A small VNC server for the active macOS desktop. It captures a display with ScreenCaptureKit, serves it through RFB, and forwards keyboard and pointer events through macOS input APIs.

It listens on loopback by default for use behind an authenticated local proxy or WebSocket bridge. It can bind directly to an explicit Tailscale address, while refusing wildcard and LAN addresses.

## Status

Early MVP:

- Primary or indexed display
- ScreenCaptureKit capture with Retina-aware input scaling
- Raw, Hextile, Tight, ZRLE, and other noVNC-compatible encodings through `rustvncserver`
- Mouse, scrolling, common keys, modifiers, and Unicode input
- Screen Recording and Accessibility permission checks/prompts
- Loopback listener by default; explicit Tailscale IPv4/IPv6 binding

Not yet implemented: VNC authentication, clipboard sync, display switching while running, login-window control, and host-application integration.

> [!WARNING]
> The RFB server currently has no built-in authentication or encryption. Keep the default loopback bind behind an authenticated proxy, or bind only to a Tailscale address protected by tailnet policy. Never expose it directly to a LAN or the public internet.

## Build

Requires macOS 12.3+, Xcode, and Rust. The repository pins Rust 1.90 because `rustvncserver` requires it.

```bash
cargo build
```

## Use

Check permissions without prompting:

```bash
cargo run -- doctor
```

Ask macOS for missing permissions:

```bash
cargo run -- doctor --request
```

Serve the primary display:

```bash
cargo run -- serve
```

Defaults to `127.0.0.1:5900`, 20 FPS, and a maximum width of 1920 pixels. Choose any port from 1 through 65535 with `--port`; host applications can select a private backend port such as 5901. See all options with:

```bash
cargo run -- help
```

To accept direct connections over Tailscale:

```bash
cargo run -- serve --listen 100.x.y.z
```

Use the Tailscale IPv4 address assigned to this Mac. Tailscale must be connected before the server starts. The CLI accepts only loopback, Tailscale's `100.64.0.0/10` IPv4 range, and its `fd7a:115c:a1e0::/48` IPv6 range; it rejects `0.0.0.0`, LAN addresses, and arbitrary public addresses.

Screen capture requires **Privacy & Security → Screen & System Audio Recording**. Remote input requires **Privacy & Security → Accessibility**. `serve` requests missing permissions by default; `doctor --request` requests them without starting the server. macOS may require the process to restart after approval.

The CLI does not change macOS Firewall, MDM, or Tailscale settings. Loopback needs no firewall exception. A direct Tailscale bind may require an inbound firewall allowance; on managed Macs, keep VNC on loopback and use an approved authenticated proxy.

For a parent-owned background process, pass `--parent-stdio`; the server exits when stdin closes. The CLI does not daemonize itself.

## Package

Create a signed background `.app` containing the Swift runtime libraries:

```bash
./scripts/package.sh
```

This uses an ad-hoc signature by default. For distribution:

```bash
SIGN_IDENTITY='Developer ID Application: …' ./scripts/package.sh
```

The result is `dist/macOS VNC Server.app`. Invoke its CLI directly:

```bash
'dist/macOS VNC Server.app/Contents/MacOS/macos-vnc-server' serve
```

A stable Developer ID signature and bundle identifier are important because macOS associates privacy grants with code identity.

## Smoke test

After granting permissions:

```bash
./scripts/smoke.sh
```

The smoke verifies the default listener is loopback-only, completes an RFB 3.8 handshake, requests a framebuffer, and confirms captured pixels are non-empty.

## Implementation

- [`screencapturekit`](https://crates.io/crates/screencapturekit) — MIT/Apache-2.0 capture bindings
- [`rustvncserver`](https://crates.io/crates/rustvncserver) — Apache-2.0 RFB server
- [`enigo`](https://crates.io/crates/enigo) — MIT input injection

`rustvncserver` 2.2.1 binds all interfaces in its public `listen(port)` API. A vendored one-method patch adds `listen_on(SocketAddr)` so this program can use an exact loopback or Tailscale address instead of a wildcard bind.

All resolved Rust dependencies are permissively licensed; there are no GPL or AGPL dependencies.

## License

Apache-2.0. Vendored components retain their original notices and licenses.
