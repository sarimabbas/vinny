#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
identity="${SIGN_IDENTITY:?set SIGN_IDENTITY to your Developer ID Application certificate}"
profile="${NOTARY_PROFILE:-developer-notary}"
version="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
arch="$(uname -m)"
app="$(pwd)/dist/Vinny.app"
archive="$(pwd)/dist/vinny-${version}-macos-${arch}.zip"
submission="$(pwd)/dist/.vinny-notarization.zip"

SIGN_IDENTITY="$identity" ./scripts/package.sh
rm -f "$submission" "$archive"
ditto -c -k --keepParent "$app" "$submission"
if [[ -n "${NOTARY_KEY_PATH:-}" ]]; then
  : "${NOTARY_KEY_ID:?set NOTARY_KEY_ID with NOTARY_KEY_PATH}"
  : "${NOTARY_ISSUER_ID:?set NOTARY_ISSUER_ID with NOTARY_KEY_PATH}"
  notary_credentials=(
    --key "$NOTARY_KEY_PATH"
    --key-id "$NOTARY_KEY_ID"
    --issuer "$NOTARY_ISSUER_ID"
  )
else
  notary_credentials=(--keychain-profile "$profile")
fi
xcrun notarytool submit "$submission" "${notary_credentials[@]}" --wait
xcrun stapler staple "$app"
xcrun stapler validate "$app"
ditto -c -k --keepParent "$app" "$archive"
rm -f "$submission"

codesign --verify --deep --strict --verbose=2 "$app"
spctl --assess --type execute --verbose=2 "$app"
shasum -a 256 "$archive"
echo "$archive"
