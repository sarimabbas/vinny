#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
identity="${SIGN_IDENTITY:?set SIGN_IDENTITY to your Developer ID Application certificate}"
profile="${NOTARY_PROFILE:-vinny-notary}"
version="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
arch="$(uname -m)"
app="$(pwd)/dist/Vinny.app"
archive="$(pwd)/dist/vinny-${version}-macos-${arch}.zip"
submission="$(pwd)/dist/.vinny-notarization.zip"

SIGN_IDENTITY="$identity" ./scripts/package.sh
rm -f "$submission" "$archive"
ditto -c -k --keepParent "$app" "$submission"
xcrun notarytool submit "$submission" --keychain-profile "$profile" --wait
xcrun stapler staple "$app"
xcrun stapler validate "$app"
ditto -c -k --keepParent "$app" "$archive"
rm -f "$submission"

codesign --verify --deep --strict --verbose=2 "$app"
spctl --assess --type execute --verbose=2 "$app"
shasum -a 256 "$archive"
echo "$archive"
