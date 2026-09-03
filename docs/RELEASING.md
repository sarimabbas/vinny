# Releasing Vinny

Vinny releases are built, signed, notarized, published, and added to Homebrew by `.github/workflows/release.yml`. Releases are deliberately started by a maintainer from protected `main`; ordinary pushes never publish a release.

## Release flow

1. Create a release branch from the latest `main`.
2. Update the version in:
   - `Cargo.toml`
   - `Cargo.lock`
   - `Info.plist` (`CFBundleShortVersionString` and `CFBundleVersion`)
3. Update documentation for user-visible changes.
4. Run:

   ```bash
   cargo fmt --check
   cargo test --locked
   cargo clippy --locked --all-targets -- -D warnings
   ./scripts/package.sh
   ```

5. Open and merge a pull request. Required CI must pass.
6. In GitHub, open **Actions → Release → Run workflow**, select `main`, and enter the version without `v`, such as `0.2.0`.
7. Wait for the unprivileged `build` job to pass, then approve the protected `release` environment deployment.
8. Confirm that signing, notarization, GitHub publishing, and the Homebrew cask update all succeed.

Equivalent command-line trigger:

```bash
gh workflow run Release --repo sarimabbas/vinny --ref main -f version=0.2.0
```

The workflow checks that the requested version matches both Cargo and the app plist. The build job has no release credentials. The signing job receives credentials only after approval and does not check out or execute repository build scripts.

## Verification

```bash
brew update
brew upgrade --cask vinny
codesign --verify --deep --strict /Applications/Vinny.app
spctl --assess --type execute --verbose=2 /Applications/Vinny.app
```

Also connect with a VNC client and verify display color, keyboard input, pointer input, and reconnect behavior.

## Credentials

All release credentials are GitHub environment secrets in `release`, restricted to `main`:

- `MACOS_CERTIFICATE_P12`
- `MACOS_CERTIFICATE_PASSWORD`
- `APPLE_API_PRIVATE_KEY_P8`
- `APPLE_API_KEY_ID`
- `APPLE_API_ISSUER_ID`
- `HOMEBREW_TAP_DEPLOY_KEY_B64`

The Homebrew deploy key is write-enabled only for `sarimabbas/homebrew-tap`. The GitHub token used to create the release is scoped to the Vinny repository and only to the release job.

Do not commit certificates, API private keys, passwords, or deploy keys. Keep the App Store Connect `.p8` in an encrypted backup because Apple allows it to be downloaded only once. Rotate a credential immediately if it may have been exposed.

## Retrying and correcting a release

The workflow can safely retry the same version after a partial failure: it replaces the release archive and updates the cask checksum. Do not delete or rewrite a published release tag. If users may have downloaded a bad release, publish a new patch version instead.

`scripts/release.sh` is a local fallback for signing and notarizing an archive. The GitHub workflow is the official publication path.
