#!/bin/bash
# Bump the version and build a Mac DMG locally.
#
#   scripts/release.sh            bump the patch version (0.1.0 -> 0.1.1)
#   scripts/release.sh 0.2.0      use this exact version
#   scripts/release.sh --dry-run  print the next version and exit
#
# Runs the core tests, bumps the version in every version file, builds a
# universal (Intel + Apple Silicon) app and a Windows setup .exe, and writes
#   releases/Payback-<version>.dmg
#   releases/Payback-<version>-windows-setup.exe
# Earlier installers are kept. The Windows build is cross-compiled with
# cargo-xwin (Tauri calls this experimental); GitHub Actions builds both on
# real machines when a version tag is pushed, and also builds the Linux
# .deb/.rpm/.AppImage, which this script doesn't.
#
# One-time setup for the Windows build:
#   brew install nsis llvm
#   rustup target add x86_64-pc-windows-msvc
#   SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk cargo install --locked cargo-xwin
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION_FILES=(
  package.json
  src-tauri/tauri.conf.json
  src-tauri/Cargo.toml
  crates/payback-core/Cargo.toml
  crates/payback-devserver/Cargo.toml
)

current=$(sed -n 's/^  "version": "\(.*\)",$/\1/p' src-tauri/tauri.conf.json)
if [[ ! "$current" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Couldn't read the current version from src-tauri/tauri.conf.json" >&2
  exit 1
fi

dry_run=false
next=""
case "${1:-}" in
  --dry-run) dry_run=true ;;
  "") ;;
  *) next="$1" ;;
esac
if [[ -z "$next" ]]; then
  if [[ ! -e "$ROOT/releases/Payback-$current.dmg" ]]; then
    # No DMG for the current version yet (first release, or a failed attempt).
    next="$current"
  else
    IFS=. read -r major minor patch <<<"$current"
    next="$major.$minor.$((patch + 1))"
  fi
fi
if [[ ! "$next" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Version must look like 1.2.3, got: $next" >&2
  exit 1
fi

dmg="$ROOT/releases/Payback-$next.dmg"
exe="$ROOT/releases/Payback-$next-windows-setup.exe"
echo "Current version: $current"
echo "Next version:    $next -> releases/$(basename "$dmg"), releases/$(basename "$exe")"
$dry_run && exit 0

if [[ -e "$dmg" || -e "$exe" ]]; then
  echo "Installers for $next already exist in releases/. Pick a different version." >&2
  exit 1
fi
for tool in makensis cargo-xwin; do
  command -v "$tool" >/dev/null || { echo "Missing $tool; see the setup notes at the top of this script." >&2; exit 1; }
done

# The default SDK on this Mac is newer than its linker understands; use 26.5
# when it's installed. Harmless elsewhere.
SDK=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk
if [[ -z "${SDKROOT:-}" && -d "$SDK" ]]; then
  export SDKROOT="$SDK"
fi

echo "==> Running core tests"
cargo test --quiet

if [[ "$next" != "$current" ]]; then
  echo "==> Setting version to $next"
  sed -i '' "s/^  \"version\": \"$current\",$/  \"version\": \"$next\",/" package.json src-tauri/tauri.conf.json
  sed -i '' "s/^version = \"$current\"$/version = \"$next\"/" src-tauri/Cargo.toml crates/payback-core/Cargo.toml crates/payback-devserver/Cargo.toml
fi
for f in "${VERSION_FILES[@]}"; do
  grep -q "$next" "$f" || { echo "Failed to update the version in $f" >&2; exit 1; }
done

echo "==> Building universal app"
npx tauri build --target universal-apple-darwin --bundles app,dmg

mkdir -p releases
cp "src-tauri/target/universal-apple-darwin/release/bundle/dmg/Payback_${next}_universal.dmg" "$dmg"
hdiutil verify -quiet "$dmg"

echo "==> Building Windows installer"
PATH="/opt/homebrew/opt/llvm/bin:$PATH" npx tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis
cp "src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/Payback_${next}_x64-setup.exe" "$exe"

echo "==> Done:"
echo "  $dmg"
echo "  $exe"
echo
echo "To publish through GitHub (builds on real Macs, Windows PCs and Linux):"
echo "  git commit -am \"Release v$next\" && git tag v$next && git push && git push origin v$next"
