#!/bin/bash

# Builds an unsigned release BranchDeck.app and replaces /Applications/BranchDeck.app (macOS only)

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"
# rustup's cargo must win over Homebrew's: the workspace manifest needs the pinned nightly
export PATH="$HOME/.cargo/bin:$PATH"

# --no-sign skips code signing and updater signing (no TAURI_SIGNING_PRIVATE_KEY needed), as in test.yml
pnpm tauri build --bundles app --no-sign

APP="${CARGO_TARGET_DIR:-$PROJECT_ROOT/target}/release/bundle/macos/BranchDeck.app"
osascript -e 'quit app "BranchDeck"' 2>/dev/null || true
rm -rf /Applications/BranchDeck.app
ditto "$APP" /Applications/BranchDeck.app
echo "Installed $APP -> /Applications/BranchDeck.app"
