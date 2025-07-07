#!/bin/bash

# Script to download and install the latest Jaeger release
# Usage: ./scripts/install-jaeger.sh [install_dir]

set -euo pipefail

# Get the script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Default installation directory
INSTALL_DIR="${1:-$PROJECT_ROOT/.tools/jaeger}"

# Detect OS and architecture
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

# Map architecture names
case "$ARCH" in
    x86_64)
        ARCH="amd64"
        ;;
    arm64|aarch64)
        ARCH="arm64"
        ;;
    *)
        echo "Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

# Map OS names for Jaeger releases
case "$OS" in
    darwin)
        OS="darwin"
        ;;
    linux)
        OS="linux"
        ;;
    *)
        echo "Unsupported OS: $OS"
        exit 1
        ;;
esac

echo "Detected OS: $OS, Architecture: $ARCH"

# Get the latest release from GitHub API
echo "Fetching latest Jaeger release..."

# For v1.71.0, Jaeger doesn't use the "v1.x.x / v2.x.x" format in tags
# They just use v1.x.x as the tag but include v2 binaries in the assets
# Let's get the tag and look for v2 assets
FULL_TAG=$(curl -s https://api.github.com/repos/jaegertracing/jaeger/releases/latest | grep '"tag_name":' | head -1 | sed -E 's/.*"tag_name":[[:space:]]*"([^"]+)".*/\1/')

if [ -z "$FULL_TAG" ]; then
    echo "Failed to fetch latest release tag"
    exit 1
fi

echo "Latest Jaeger release: $FULL_TAG"

# Get the list of assets and find the v2 binary for our platform
# The v2 binaries are named like: jaeger-2.0.0-darwin-amd64.tar.gz
ASSETS_URL="https://api.github.com/repos/jaegertracing/jaeger/releases/tags/${FULL_TAG}"
V2_FILENAME=$(curl -s "$ASSETS_URL" | grep -o "jaeger-2\.[0-9.]*-${OS}-${ARCH}\.tar\.gz" | head -1)

if [ -z "$V2_FILENAME" ]; then
    echo "Failed to find Jaeger v2 binary for ${OS}-${ARCH}"
    echo "Trying default v2 version..."
    V2_FILENAME="jaeger-2.0.0-${OS}-${ARCH}.tar.gz"
fi

# Extract version from filename
V2_VERSION=$(echo "$V2_FILENAME" | sed -E 's/jaeger-([0-9.]+)-.*/\1/')
echo "Using Jaeger v2 version: $V2_VERSION"

# Construct download URL for v2 binary
FILENAME="$V2_FILENAME"
DOWNLOAD_URL="https://github.com/jaegertracing/jaeger/releases/download/${FULL_TAG}/${FILENAME}"

echo "Downloading from: $DOWNLOAD_URL"

# Create install directory
mkdir -p "$INSTALL_DIR"

# Download the archive
TEMP_FILE="/tmp/${FILENAME}"
echo "Downloading Jaeger..."
if ! curl -L -o "$TEMP_FILE" "$DOWNLOAD_URL"; then
    echo "Failed to download Jaeger"
    exit 1
fi

# Extract the archive
echo "Extracting to $INSTALL_DIR..."
# Create a temporary extraction directory
TEMP_EXTRACT="/tmp/jaeger-extract-$$"
mkdir -p "$TEMP_EXTRACT"
tar -xzf "$TEMP_FILE" -C "$TEMP_EXTRACT"

# Find the extracted directory and move its contents to the install directory
EXTRACTED_DIR=$(find "$TEMP_EXTRACT" -type d -name "jaeger-*" -mindepth 1 -maxdepth 1 | head -1)
if [ -n "$EXTRACTED_DIR" ]; then
    # Move contents to install directory
    mv "$EXTRACTED_DIR"/* "$INSTALL_DIR"/ 2>/dev/null || true
    # Also handle if files are directly in temp extract
    mv "$TEMP_EXTRACT"/* "$INSTALL_DIR"/ 2>/dev/null || true
fi

# Clean up
rm -rf "$TEMP_FILE" "$TEMP_EXTRACT"

# Make binaries executable (they should already be, but just in case)
chmod +x "$INSTALL_DIR"/*

echo "✅ Jaeger v${V2_VERSION} has been successfully installed to $INSTALL_DIR"
echo ""
echo "Available binaries:"
find "$INSTALL_DIR" -type f -perm +111 -maxdepth 1 -exec basename {} \; | sed 's/^/  - /'
echo ""
echo "To run Jaeger v2:"
echo "  $INSTALL_DIR/jaeger --config /path/to/config.yaml"
echo ""
echo "Or use the run-jaeger.sh script which creates the config automatically."
