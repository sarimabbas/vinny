# Contributing to Vinny

## Before opening an issue

- Use [GitHub issues](https://github.com/sarimabbas/vinny/issues/new/choose) for usage questions.
- Report security vulnerabilities privately as described in [SECURITY.md](SECURITY.md).
- Security reports should identify whether the listener used unauthenticated, legacy VNC, or VeNCrypt/X.509 TLS security.

## Development

Vinny requires macOS 12.3 or newer, Xcode, and the Rust version pinned in `rust-toolchain.toml`.

```bash
cargo build
./scripts/package.sh
open dist/Vinny.app
```

Before submitting a change, run the same checks as CI:

```bash
cargo fmt --check
cargo test --locked
cargo test --manifest-path vendor/rustvncserver/Cargo.toml
cargo test --manifest-path vendor/rfb-encodings/Cargo.toml --lib
cargo clippy --locked --all-targets -- -D warnings
```

For app-level changes, also run:

```bash
./scripts/package.sh
./scripts/smoke.sh
```

The smoke test requires Screen Recording and Accessibility permission for the packaged app. It launches `Vinny.app`, verifies that port `5900` is loopback-only, completes an RFB 3.8 handshake, requests a framebuffer, and confirms captured pixels are non-empty.

### Package locally

For an ad-hoc signed development build:

```bash
./scripts/package.sh
```

For a Developer ID build:

```bash
SIGN_IDENTITY='Developer ID Application: …' ./scripts/package.sh
```

Keep the Developer ID identity and `run.lil.vinny` bundle identifier stable. macOS ties privacy grants to that identity. See [Releasing](RELEASING.md#local-release-archive) for local notarization and archives.

### Qualify the user connection guide

When changing connection behavior or the [first connection guide](first-connection.md), test from a second computer. Check permissions, keyboard and pointer input, view-only mode after restart, display selection, and reconnection. Include the macOS, Vinny, and viewer versions in the test results. The local smoke test covers capture and the RFB handshake; these checks cover the viewer and remote input.

## Pull requests

1. Branch from the latest `main`.
2. Keep the change focused and include tests for behavior changes.
3. Update user-facing documentation when behavior changes.
4. Open a pull request. CI must pass before merge.
5. Use a squash merge.

Branch protection blocks direct pushes, force pushes, and deletion of `main`. See [RELEASING.md](RELEASING.md) for the release process.
