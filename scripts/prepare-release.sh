#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Prepare Vinny's version files for a release.

Usage:
  ./scripts/prepare-release.sh --version X.Y.Z [--dry-run]
  ./scripts/prepare-release.sh --help

Options:
  --version X.Y.Z  Required release version without a leading v.
  --dry-run        Show the planned version and build number without editing files.
  -h, --help       Show this help.

The script updates Cargo.toml, Cargo.lock, and Info.plist together. It refuses
to run when any of those files already has uncommitted changes.
EOF
}

version=""
dry_run=false
while (($#)); do
  case "$1" in
    --version)
      [[ $# -ge 2 ]] || { echo "error: --version requires a value" >&2; exit 2; }
      version="$2"
      shift 2
      ;;
    --dry-run)
      dry_run=true
      shift
      ;;
    -h|--help|help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[[ -n "$version" ]] || { echo "error: --version is required" >&2; exit 2; }
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  echo "error: version must have the form X.Y.Z" >&2
  exit 2
}

cd "$(dirname "$0")/.."
files=(Cargo.toml Cargo.lock Info.plist)
if ! git diff --quiet -- "${files[@]}" || ! git diff --cached --quiet -- "${files[@]}"; then
  echo "error: commit or stash existing version-file changes first" >&2
  exit 1
fi

current="$(python3 - <<'PY'
import tomllib
with open("Cargo.toml", "rb") as file:
    print(tomllib.load(file)["package"]["version"])
PY
)"
python3 - "$current" "$version" <<'PY'
import sys
current = tuple(map(int, sys.argv[1].split(".")))
requested = tuple(map(int, sys.argv[2].split(".")))
if requested <= current:
    raise SystemExit(f"error: version must be newer than {sys.argv[1]}")
PY

build="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' Info.plist)"
[[ "$build" =~ ^[0-9]+$ ]] || { echo "error: CFBundleVersion must be an integer" >&2; exit 1; }
next_build=$((build + 1))

printf 'Vinny %s -> %s (build %s -> %s)\n' "$current" "$version" "$build" "$next_build"
if $dry_run; then
  exit 0
fi

backup="$(mktemp -d)"
for file in "${files[@]}"; do cp "$file" "$backup/$(basename "$file")"; done
restore_on_error() {
  status=$?
  if ((status != 0)); then
    for file in "${files[@]}"; do cp "$backup/$(basename "$file")" "$file"; done
    echo "error: release preparation failed; restored version files" >&2
  fi
  rm -rf "$backup"
  exit "$status"
}
trap restore_on_error EXIT

python3 - "$current" "$version" <<'PY'
from pathlib import Path
import sys
path = Path("Cargo.toml")
old = f'version = "{sys.argv[1]}"'
new = f'version = "{sys.argv[2]}"'
text = path.read_text()
if text.count(old) != 1:
    raise SystemExit("error: could not identify the package version in Cargo.toml")
path.write_text(text.replace(old, new, 1))
PY
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" Info.plist
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $next_build" Info.plist
cargo check --quiet

[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' Info.plist)" == "$version" ]]
[[ "$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')" == "$version" ]]

trap - EXIT
rm -rf "$backup"
echo "Updated Cargo.toml, Cargo.lock, and Info.plist. Review the diff, then open a release PR."
