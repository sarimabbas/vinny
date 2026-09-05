# Vinny

[![CI](https://github.com/sarimabbas/vinny/actions/workflows/ci.yml/badge.svg)](https://github.com/sarimabbas/vinny/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/sarimabbas/vinny)](https://github.com/sarimabbas/vinny/releases/latest)

**An open-source VNC server for macOS with control over what you share.**

Choose a display, set its maximum width and frame rate, and decide whether viewers can use the keyboard and mouse. Run separate listeners for different displays, all from a native menu-bar app.

[Download v0.2.4 for Apple Silicon](https://github.com/sarimabbas/vinny/releases/download/v0.2.4/vinny-0.2.4-macos-arm64.zip) · [First connection guide](docs/first-connection.md) · [Website](https://vinny.lil.run)

## When should I use Vinny?

Use Vinny when you want to:

- Share a selected display through its own VNC listener.
- Set capture width and frame rate for each listener.
- Switch a listener between remote control and view-only access.
- Inspect or extend a small Rust and native macOS application.

For ordinary Mac-to-Mac remote access, built-in Screen Sharing may already meet your needs. Vinny is useful when you want these controls together in one app. See [Vinny vs built-in Screen Sharing](docs/vinny-vs-screen-sharing.md).

## Install

Requires macOS 12.3 or newer. The published download above is for **Apple Silicon (ARM64)**. For other architectures, see the [build instructions](docs/CONTRIBUTING.md#development).

With Homebrew:

```bash
brew install --cask sarimabbas/tap/vinny
```

Or download the ZIP above, extract it, and move `Vinny.app` to Applications. [All releases and checksums](https://github.com/sarimabbas/vinny/releases).

## Get your first remote connection

You need Vinny on the Mac being shared, [TigerVNC Viewer](https://tigervnc.org/) on the other computer, and SSH access to the Mac.

1. Open Vinny from Applications. Grant **Screen Recording** and **Accessibility** when prompted. Both are required for enabled servers to start.
2. Keep the first server on **`127.0.0.1:5900`**, choose your display, and leave **Encrypted + password** off for this SSH-tunnel recipe. Check that the server is enabled and its status is **listening**.
3. Enable **Remote Login** on the Mac for your account. From the other computer, open a terminal and run the following, replacing `YOUR_USER` and `YOUR_MAC` with the account and address shown in the Mac's Remote Login settings:

   ```bash
   ssh -N -o ExitOnForwardFailure=yes -L 127.0.0.1:15900:127.0.0.1:5900 YOUR_USER@YOUR_MAC
   ```

4. Leave that terminal open. In TigerVNC Viewer, connect to **`127.0.0.1::15900`**. The double colon specifies an exact port.
5. Confirm you see the chosen display. Try moving the pointer and typing into an empty text document. To make the session view-only, turn off **Allow keyboard and mouse**, choose **Apply & restart**, and reconnect.

SSH encrypts the connection between computers and authenticates access to the tunnel. The VNC listener remains unauthenticated locally, so other users and processes on the Mac can still access it. Keep it on loopback for this recipe. Use a Mac and viewer computer you trust.

For Remote Login setup, viewer security options, port conflicts, and connection errors, follow the [full first connection guide](docs/first-connection.md).

**Connected successfully?** [Star Vinny on GitHub](https://github.com/sarimabbas/vinny) to support the project, or [tell us what worked and what got in the way](https://github.com/sarimabbas/vinny/issues/new/choose).

## Everyday use

- [Share a selected display](docs/share-selected-display.md), including separate listeners and view-only access.
- Closing the window leaves the servers running and hides the Dock icon. Reopen the window from the menu bar. Quit Vinny to stop the app.
- Settings persist between launches. New servers use the next available configured port starting at `5900`.

## Connection security

Vinny defaults to an **unauthenticated loopback listener**. For access from another computer, use the SSH-tunnel guide above, or configure encrypted connections with a compatible viewer.

**Encrypted + password** supports VeNCrypt/X509Plain, including TigerVNC. Vinny creates a self-signed certificate when a server starts. Its fingerprint changes when the server is recreated, so use an authenticated tunnel when you need a stable, verified endpoint.

The optional **Legacy authentication (unencrypted)** setting allows legacy VNC clients such as macOS Screen Sharing. It limits passwords to eight bytes and does **not** encrypt screen contents or input. Use it through a secure tunnel. View-only access still exposes screen and outgoing clipboard contents.

See the [threat model](docs/threat-model.md) and [security policy](docs/SECURITY.md).

## Under the hood

Vinny captures displays with ScreenCaptureKit, serves them over standard RFB, and forwards keyboard and pointer events through macOS input APIs. Capture and input account for Retina scaling.

It supports RFB 3.3 through 3.8, common framebuffer encodings, framebuffer resizing, cursor metadata, extended key events, and clipboard sync. Each server accepts at most eight clients, with a 10-second handshake timeout.

- [`screencapturekit`](https://crates.io/crates/screencapturekit): MIT/Apache-2.0 capture bindings
- [`rustvncserver`](https://crates.io/crates/rustvncserver): Apache-2.0 RFB server, vendored at 2.2.1
- [`enigo`](https://crates.io/crates/enigo): MIT input injection

## Development and documentation

[Build, package, and smoke test](docs/CONTRIBUTING.md) · [Release and notarize](docs/RELEASING.md) · [All documentation](docs/README.md)

## License

[Apache-2.0](LICENSE). Vendored components retain their original notices and licenses.
