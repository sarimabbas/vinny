#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
app="$(pwd)/dist/macOS VNC Server.app"
identity="${SIGN_IDENTITY:--}"

cargo build --release
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Frameworks"
cp Info.plist "$app/Contents/Info.plist"
cp target/release/macos-vnc-server "$app/Contents/MacOS/"

xcrun swift-stdlib-tool --copy \
  --platform macosx \
  --scan-executable "$app/Contents/MacOS/macos-vnc-server" \
  --destination "$app/Contents/Frameworks" \
  --sign "$identity"
find "$app/Contents/Frameworks" -type f -name '*.original' -delete

sign=(--force --sign "$identity")
if [[ "$identity" != "-" ]]; then
  sign+=(--options runtime --timestamp)
fi
codesign "${sign[@]}" "$app/Contents/MacOS/macos-vnc-server"
codesign "${sign[@]}" "$app"
codesign --verify --strict --verbose=2 "$app"

echo "$app"
