# Releasing Vinny

`.github/workflows/release.yml` builds, signs, notarizes, publishes, and updates Homebrew. It runs only when a maintainer starts it from protected `main`.

## Release flow

1. Create a release branch from the latest `main`.
2. Update Cargo, the lockfile, and the app plist together:

   ```bash
   ./scripts/prepare-release.sh --version 0.2.0
   ```

   The script increments `CFBundleVersion`. It stops if a version file already has uncommitted changes. Add `--dry-run` to preview the change.
3. Update documentation for user-visible changes.
4. Run:

   ```bash
   cargo fmt --check
   cargo test --locked
   cargo test --manifest-path vendor/rustvncserver/Cargo.toml
   cargo test --manifest-path vendor/rfb-encodings/Cargo.toml --lib
   cargo clippy --locked --all-targets -- -D warnings
   ./scripts/package.sh
   ```

5. Open and merge a pull request. Required CI must pass.
6. In GitHub, open **Actions → Release → Run workflow**, select `main`, and enter the version without `v`, such as `0.2.0`.
7. Wait for the unprivileged `build` job to pass, then approve the protected `release` environment deployment.
8. Confirm that signing, notarization, and GitHub publishing succeed.
9. Open the Homebrew pull-request link from the workflow summary. Review and merge it to publish the cask update.
10. After the release archive is published successfully, open a documentation pull request updating the pinned ARM64 download version and asset URL in `README.md` and `website/index.html`. Check any guide links to the archive too. Verify that each URL points to the published asset before merging. Do not update these links ahead of publication: versioned filenames have no stable download alias.

Equivalent command-line trigger:

```bash
gh workflow run Release --repo sarimabbas/vinny --ref main -f version=0.2.0
```

The workflow rejects a version that does not match Cargo and the app plist, or one that has already been published.

Release archives, checksums, and tags are immutable. Updating the Homebrew cask requires a pull request.

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

The Homebrew key can write only to `sarimabbas/homebrew-tap`. The release token can write only to the Vinny repository.

Never commit certificates, private keys, passwords, or deploy keys. Apple provides the App Store Connect `.p8` only once, so keep an encrypted backup. Rotate any credential that may have leaked.

## Failures and corrections

Once GitHub has created a release, do not reuse its version, replace its archive, or rewrite its tag.

- If signing, notarization, or release creation fails before publication, fix the workflow and run the same version again.
- If only the Homebrew branch or pull request fails, update the cask from the published archive. Do not rebuild the release.
- If a published binary is wrong, publish a new patch version.

Use `scripts/release.sh` to sign and notarize a local archive. Publish releases through the GitHub workflow.

## Local release archive

To sign, notarize, staple, and archive a release with the default `developer-notary` keychain profile:

```bash
SIGN_IDENTITY='Developer ID Application: …' ./scripts/release.sh
```

Set `NOTARY_PROFILE` to use a differently named keychain profile. The archive and its SHA-256 checksum are written to `dist/`. Keep the Developer ID identity and `run.lil.vinny` bundle identifier stable so macOS privacy grants remain associated with the app.

Use the protected GitHub workflow above to publish releases.
