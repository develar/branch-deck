#!/bin/bash

# Builds the app icons from the Icon Composer document src-tauri/icons/BranchDeck.icon (macOS, Xcode 26+)
#
# macOS 26+ puts an app that has only an .icns into a grey squircle; the Liquid Glass icon is Assets.car with
# CFBundleIconName. The Tauri bundler compiles a .icon only when `actool` 26+ is in PATH and skips it silently
# otherwise, so the compiled Assets.car is committed and bundle.icon lists it. The .icns, .ico and PNGs (macOS before
# 26, Windows, Linux) are the Default rendition of the same document on Apple's grid: an 824 px body in 1024.
# Run it after every change of the document and commit what it writes.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ICONS="$PROJECT_ROOT/src-tauri/icons"
DOCUMENT="$ICONS/BranchDeck.icon"

# xcode-select may point to the Command Line Tools, which have neither actool nor Icon Composer
if [[ -z "${DEVELOPER_DIR:-}" && "$(xcode-select -p)" != *Xcode*.app* ]]; then
  export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
fi
ACTOOL="$(xcrun --find actool)"
ICTOOL="${DEVELOPER_DIR:-$(xcode-select -p)}/../Applications/Icon Composer.app/Contents/Executables/ictool"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# the arguments of the Tauri bundler (create_assets_car_file), the icon named after the document
mkdir "$WORK/car"
"$ACTOOL" "$DOCUMENT" --compile "$WORK/car" \
  --output-format human-readable-text --notices --warnings \
  --output-partial-info-plist "$WORK/car/assetcatalog_generated_info.plist" \
  --app-icon BranchDeck --include-all-app-icons \
  --enable-on-demand-resources NO --development-region en \
  --target-device mac --minimum-deployment-target 26.0 --platform macosx >"$WORK/actool.log" || { cat "$WORK/actool.log"; exit 1; }
cp "$WORK/car/Assets.car" "$ICONS/Assets.car"

"$ICTOOL" "$DOCUMENT" --export-image --output-file "$WORK/icon.png" \
  --platform macOS --rendition Default --width 824 --height 824 --scale 1 >/dev/null
sips --padToHeightWidth 1024 1024 "$WORK/icon.png" >/dev/null

cd "$PROJECT_ROOT"
pnpm tauri icon "$WORK/icon.png" --output "$WORK/out" >"$WORK/tauri-icon.log" 2>&1 || { cat "$WORK/tauri-icon.log"; exit 1; }
for file in icon.icns icon.ico icon.png 32x32.png 128x128.png 128x128@2x.png; do
  cp "$WORK/out/$file" "$ICONS/$file"
done
echo "Wrote $ICONS/Assets.car and the icon files of tauri.conf.json"
