#!/bin/bash

# Wrapper script for the Rust test repo generator
# Usage: ./scripts/create-test-repo.sh [path]

set -e

# Default path if not provided
  REPO_PATH="${1:-./test-repo}"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}Building test repo generator...${NC}"

# Build the generator
cd "$(dirname "$0")/.."
cargo build --release --bin generate-test-repo -p test-utils

echo -e "${GREEN}✓ Build complete${NC}"
echo -e "${BLUE}Generating test repository...${NC}"

# Run the generator
./target/release/generate-test-repo --output "$REPO_PATH" --verbose

echo -e "\n${GREEN}✓ Test repository created at: $REPO_PATH${NC}"