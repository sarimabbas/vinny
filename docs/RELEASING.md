# Releasing Vinny

Vinny releases are built, signed, notarized, published, and added to Homebrew by `.github/workflows/release.yml`. Releases are deliberately started by a maintainer from protected `main`; ordinary pushes never publish a release.

## Release flow

1. Create a release branch from the latest `main`.
2. Update Cargo, the lockfile, and the app plist together:

   ```bash
   ./scripts/prepare-release.sh --version 0.2.0
   ```

   Use `--dry-run` first if desired. The script also increments `CFBundleVersion` and refuses to overwrite existing changes to version files.
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
8. Confirm that signing, notarization, and GitHub publishing succeed.
9. Follow the workflow summary link to open the generated Homebrew tap branch as a pull request. Review and merge it to publish the cask update; GitHub deletes the branch automatically after merge.

Equivalent command-line trigger:

```bash
gh workflow run Release --repo sarimabbas/vinny --ref main -f version=0.2.0
```

The workflow checks that the requested version matches both Cargo and the app plist and has never been published. The build job has no release credentials. The signing job receives credentials only after approval and does not check out or execute repository build scripts.

GitHub release archives and their checksum files are immutable. The workflow never replaces an asset or reuses a tag. The Homebrew deploy key can prepare a branch in the tap, but protected `main` accepts the update only through a reviewed pull request.

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

## Failures and corrections

A version cannot be rerun after its GitHub release has been created. Never replace an archive or delete or rewrite a published tag.

- If signing, notarization, or release creation fails before publication, fix the workflow and run the same version again.
- If only the Homebrew branch or pull request fails, update the cask from the already-published archive; do not rebuild the release.
- If a published binary is wrong, publish a new patch version.

`scripts/release.sh` is a local fallback for signing and notarizing an archive. The GitHub workflow is the official publication path.
