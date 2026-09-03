# Contributing to Vinny

Thanks for helping improve Vinny.

## Before opening an issue

- Use [GitHub Discussions](https://github.com/sarimabbas/vinny/discussions) for usage questions if discussions are enabled.
- Report security vulnerabilities privately as described in [SECURITY.md](SECURITY.md).
- Remember that VNC authentication and encryption are not currently implemented; that limitation is documented rather than a new vulnerability.

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
cargo test --manifest-path vendor/rustvncserver/Cargo.toml --locked
cargo test --manifest-path vendor/rfb-encodings/Cargo.toml --locked --lib
cargo clippy --locked --all-targets -- -D warnings
```

For app-level changes, also run:

```bash
./scripts/package.sh
./scripts/smoke.sh
```

The smoke test requires Screen Recording and Accessibility permission for the packaged app.

## Pull requests

1. Branch from the latest `main`.
2. Keep the change focused and include tests for behavior changes.
3. Update user-facing documentation when behavior changes.
4. Open a pull request. CI must pass before merge.
5. Use a squash merge so `main` remains linear.

Direct pushes, force pushes, and deletion of `main` are blocked. Maintainers release only from protected `main`; see [RELEASING.md](RELEASING.md).
