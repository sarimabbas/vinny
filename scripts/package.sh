#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
app="$(pwd)/dist/Vinny.app"
identity="${SIGN_IDENTITY:--}"

cargo build --release
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Frameworks" "$app/Contents/Resources"
cp Info.plist "$app/Contents/Info.plist"
cp target/release/vinny "$app/Contents/MacOS/"
cp assets/vinny-front.png "$app/Contents/Resources/OnboardingMascot.png"
cp assets/Vinny.icns "$app/Contents/Resources/"
cp website/public/fonts/kinder-child-kawaii-bubble.otf "$app/Contents/Resources/"
cp website/public/fonts/maple-mono-regular.ttf "$app/Contents/Resources/"

xcrun swift-stdlib-tool --copy \
  --platform macosx \
  --scan-executable "$app/Contents/MacOS/vinny" \
  --destination "$app/Contents/Frameworks" \
  --sign "$identity"
find "$app/Contents/Frameworks" -type f -name '*.original' -delete

sign=(--force --sign "$identity")
if [[ "$identity" != "-" ]]; then
  sign+=(--options runtime --timestamp)
fi
codesign "${sign[@]}" "$app/Contents/MacOS/vinny"
codesign "${sign[@]}" "$app"
codesign --verify --strict --verbose=2 "$app"

echo "$app"
