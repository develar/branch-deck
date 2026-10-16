#!/bin/sh
# Installs or upgrades Branch Deck from the GitHub release (macOS on Apple silicon, Linux x86_64):
#   curl -fsSL https://raw.githubusercontent.com/develar/branch-deck/master/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/develar/branch-deck/master/install.sh | sh -s -- --with-cli
#
# curl sets no com.apple.quarantine attribute, so macOS doesn't run Gatekeeper on the unsigned app, as for the
# app's own updates. The asset URLs come from the release's latest.json, the file the updater reads.
#
# Options:
#   --version <x.y.z>  install this release instead of the latest one
#   --with-cli         also install the Agent CLI (`bd install`: `bd` in ~/.local/bin and the agent skills)
# Environment:
#   BRANCH_DECK_INSTALL_DIR  macOS: the directory for BranchDeck.app (default /Applications, else ~/Applications);
#                            Linux AppImage: the directory for the branch-deck binary (default ~/.local/bin)
#   BRANCH_DECK_FORMAT       Linux: deb, rpm or appimage (default: deb with apt-get, rpm with dnf or zypper, else appimage)
#   BRANCH_DECK_MANIFEST     the URL of latest.json instead of the release's (tests: a file:// URL of a local build)

set -eu

REPO="develar/branch-deck"
version=""
with_cli=false

say() {
  printf '%s\n' "$*"
}

fail() {
  printf 'Cannot install Branch Deck: %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Installs or upgrades Branch Deck from its GitHub release.

Usage: install.sh [--version <x.y.z>] [--with-cli]
  --version <x.y.z>  install this release instead of the latest one
  --with-cli         also install the Agent CLI (bd in ~/.local/bin and the agent skills)

Environment:
  BRANCH_DECK_INSTALL_DIR  macOS: the directory for BranchDeck.app; Linux: the directory for the AppImage
  BRANCH_DECK_FORMAT       Linux: deb, rpm or appimage
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version)
      [ $# -ge 2 ] || fail "--version needs a version, e.g. --version 0.11.0"
      version="${2#v}"
      shift 2
      ;;
    --version=*)
      version="${1#--version=}"
      version="${version#v}"
      shift
      ;;
    --with-cli)
      with_cli=true
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      fail "unknown option $1"
      ;;
  esac
done

command -v curl >/dev/null 2>&1 || fail "curl is not installed"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

if [ -n "${BRANCH_DECK_MANIFEST:-}" ]; then
  manifest_url="$BRANCH_DECK_MANIFEST"
elif [ -n "$version" ]; then
  manifest_url="https://github.com/$REPO/releases/download/v$version/latest.json"
else
  manifest_url="https://github.com/$REPO/releases/latest/download/latest.json"
fi
curl -fsSL "$manifest_url" -o "$tmp/latest.json" || fail "no release manifest at $manifest_url"
# one line, so that a platform entry can be cut out with sed; the notes are a JSON string with escaped quotes
tr '\n' ' ' <"$tmp/latest.json" >"$tmp/latest.line"
release="$(sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' "$tmp/latest.line")"
[ -n "$release" ] || fail "the release manifest $manifest_url has no version"

# The download URL of a platform key of latest.json (darwin-aarch64-app, linux-x86_64-deb, …)
asset_url() {
  sed -n "s/.*\"$1\": *{[^}]*\"url\": *\"\\([^\"]*\\)\".*/\\1/p" "$tmp/latest.line"
}

download() {
  url="$(asset_url "$1")"
  [ -n "$url" ] || fail "release $release has no $1 build"
  say "Downloading ${url##*/}"
  curl -fL --progress-bar "$url" -o "$tmp/${url##*/}" || fail "download of $url failed"
  asset="$tmp/${url##*/}"
}

install_macos() {
  [ "$(uname -m)" = "arm64" ] || fail "Branch Deck for macOS is built for Apple silicon only"
  if [ -n "${BRANCH_DECK_INSTALL_DIR:-}" ]; then
    dir="$BRANCH_DECK_INSTALL_DIR"
  elif [ -w /Applications ]; then
    dir="/Applications"
  else
    dir="$HOME/Applications"
  fi
  mkdir -p "$dir" || fail "cannot create $dir"

  download darwin-aarch64-app
  mkdir "$tmp/app"
  tar -xzf "$asset" -C "$tmp/app" || fail "cannot unpack ${asset##*/}"
  [ -d "$tmp/app/BranchDeck.app" ] || fail "${asset##*/} has no BranchDeck.app"
  # curl adds no quarantine attribute, but a proxy or a management tool can
  xattr -dr com.apple.quarantine "$tmp/app/BranchDeck.app" 2>/dev/null || true

  app="$dir/BranchDeck.app"
  binary="$app/Contents/MacOS/branch-deck"
  # an upgrade quits the running app (this copy only, not a development build) and starts it again afterwards
  was_running=false
  if pgrep -f "^$binary" >/dev/null 2>&1; then
    was_running=true
    osascript -e "tell application \"$app\" to quit" >/dev/null 2>&1 || true
    i=0
    while pgrep -f "^$binary" >/dev/null 2>&1 && [ $i -lt 20 ]; do
      sleep 0.5
      i=$((i + 1))
    done
  fi
  # copy next to the old app first, so a failed copy never leaves half an app
  rm -rf "$app.new"
  ditto "$tmp/app/BranchDeck.app" "$app.new" || fail "cannot copy BranchDeck.app to $dir"
  rm -rf "$app"
  mv "$app.new" "$app"
  installed="$app"
  [ "$was_running" = false ] || open "$app" || true
}

# root runs the package manager itself, everyone else through sudo
as_root() {
  if [ "$(id -u)" -eq 0 ]; then
    "$@"
  else
    command -v sudo >/dev/null 2>&1 || fail "installing a package needs root, and sudo is not installed"
    sudo "$@"
  fi
}

install_linux() {
  [ "$(uname -m)" = "x86_64" ] || fail "Branch Deck for Linux is built for x86_64 only"
  format="${BRANCH_DECK_FORMAT:-}"
  if [ -z "$format" ]; then
    if command -v apt-get >/dev/null 2>&1; then
      format=deb
    elif command -v dnf >/dev/null 2>&1 || command -v zypper >/dev/null 2>&1; then
      format=rpm
    else
      format=appimage
    fi
  fi

  case "$format" in
    deb)
      download linux-x86_64-deb
      # apt-get resolves the dependencies (WebKitGTK) of a local package given by path
      as_root env DEBIAN_FRONTEND=noninteractive apt-get install -y "$asset" || fail "apt-get could not install ${asset##*/}"
      installed="/usr/bin/branch-deck"
      binary="$installed"
      ;;
    rpm)
      download linux-x86_64-rpm
      if command -v dnf >/dev/null 2>&1; then
        as_root dnf install -y "$asset" || fail "dnf could not install ${asset##*/}"
      else
        as_root zypper --non-interactive install --allow-unsigned-rpm "$asset" || fail "zypper could not install ${asset##*/}"
      fi
      installed="/usr/bin/branch-deck"
      binary="$installed"
      ;;
    appimage)
      download linux-x86_64-appimage
      dir="${BRANCH_DECK_INSTALL_DIR:-$HOME/.local/bin}"
      mkdir -p "$dir" || fail "cannot create $dir"
      chmod +x "$asset"
      mv "$asset" "$dir/branch-deck"
      installed="$dir/branch-deck"
      binary="$installed"
      install_desktop_entry
      ;;
    *)
      fail "unknown BRANCH_DECK_FORMAT $format: use deb, rpm or appimage"
      ;;
  esac
}

# The menu entry and the icon of the AppImage; the packages bring their own
install_desktop_entry() {
  data="${XDG_DATA_HOME:-$HOME/.local/share}"
  mkdir -p "$data/applications" "$data/icons/hicolor/128x128/apps"
  # the AppImage extracts its icon without FUSE
  if (cd "$tmp" && "$installed" --appimage-extract '*.png' >/dev/null 2>&1); then
    icon="$(find "$tmp/squashfs-root" -maxdepth 1 -name '*.png' | head -n 1)"
    [ -z "$icon" ] || cp "$icon" "$data/icons/hicolor/128x128/apps/branch-deck.png"
  fi
  cat >"$data/applications/branch-deck.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Branch Deck
Comment=Virtual branches from commit prefixes
Exec="$installed" %U
Icon=branch-deck
Terminal=false
Categories=Development;
EOF
}

case "$(uname -s)" in
  Darwin) install_macos ;;
  Linux) install_linux ;;
  *) fail "use install.ps1 on Windows, or download a build from https://github.com/$REPO/releases" ;;
esac

# Whether version $1 is $2 or newer (numeric major.minor.patch)
version_at_least() {
  [ "$(printf '%s\n%s\n' "$1" "$2" | sort -t. -k1,1n -k2,2n -k3,3n | head -n 1)" = "$2" ]
}

# the Agent CLI is in the app binary since this release; an older binary ignores `cli` and starts the app
CLI_SINCE="0.12.0"
has_cli=false
if version_at_least "$release" "$CLI_SINCE"; then
  has_cli=true
fi

# runs the CLI of the installed binary; an AppImage without FUSE runs extracted
cli() {
  APPIMAGE_EXTRACT_AND_RUN=1 "$binary" cli "$@"
}

if [ "$has_cli" = true ]; then
  # also the proof that the installed binary starts; its last line says why not (a missing library of an AppImage)
  if ! started="$(cli version 2>&1)"; then
    reason="$(printf '%s\n' "$started" | tail -n 1)"
    if [ "${format:-}" = appimage ]; then
      reason="$reason (the AppImage uses the WebKitGTK 4.1 and GTK 3 libraries of the system)"
    fi
    fail "the installed $binary does not start: $reason"
  fi
fi
say "Installed Branch Deck $release to $installed"

# The newest Git of the ones the app finds: on PATH, else Homebrew's (an app started from the Dock has no shell PATH)
git_version=""
for git in git /opt/homebrew/bin/git /usr/local/bin/git; do
  found="$("$git" --version 2>/dev/null | sed -n 's/^git version \([0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\).*/\1/p')" || true
  if [ -n "$found" ] && { [ -z "$git_version" ] || version_at_least "$found" "$git_version"; }; then
    git_version="$found"
  fi
done
if [ -z "$git_version" ] || ! version_at_least "$git_version" "2.54.0"; then
  if [ -z "$git_version" ]; then
    say "Git is not installed: Branch Deck requires Git 2.54 or newer. Install it with"
  else
    say "Git $git_version is too old: Branch Deck requires Git 2.54 or newer. Upgrade it with"
  fi
  if [ "$(uname -s)" = "Darwin" ]; then
    say "  brew install git"
  elif command -v apt-get >/dev/null 2>&1; then
    say "  sudo add-apt-repository ppa:git-core/ppa && sudo apt install git"
  else
    say "  your package manager"
  fi
fi

if [ "$has_cli" = false ]; then
  [ "$with_cli" = false ] || say "Branch Deck $release has no Agent CLI yet: it comes with $CLI_SINCE"
elif [ "$with_cli" = true ]; then
  cli install >"$tmp/install.json" || fail "the Agent CLI could not be installed"
  tr -d '\n' <"$tmp/install.json" >"$tmp/install.line"
  bd="$(sed -n 's/.*"bd": *"\([^"]*\)".*/\1/p' "$tmp/install.line")"
  say "Installed the Agent CLI as $bd, and the branch-deck skill for coding agents"
  if grep -q '"on_path": *false' "$tmp/install.line"; then
    say "${bd%/*} is not on PATH: add it in your shell profile, e.g. export PATH=\"\$HOME/.local/bin:\$PATH\""
  fi
else
  say "Agent CLI for coding agents: run the installer with --with-cli, or Help → Install Agent CLI… in the app"
fi
