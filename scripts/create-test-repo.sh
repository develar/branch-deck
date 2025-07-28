#!/bin/bash

# Wrapper script for the Rust test repo generator
# Usage: ./scripts/create-test-repo.sh [path]

set -e

# Default path if not provided
REPO_PATH="${1:-./test-repo}"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Build and run the generator
cd "$(dirname "$0")/.."

echo -e "${BLUE}Building and generating test repository...${NC}"

# Run the generator
cargo run --bin generate-test-repo -p test-utils -- --output "$REPO_PATH"

echo -e "\n${GREEN}✓ Test repository created at: $REPO_PATH${NC}"