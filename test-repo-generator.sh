#!/bin/bash
# Script to generate a test repository using the test_repo_generator

set -e

# Get the directory where the script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Default output directory
OUTPUT_DIR="${1:-$SCRIPT_DIR/test-repo-output}"

echo "Generating test repository at: $OUTPUT_DIR"

# Build and run the test repo generator
cd "$SCRIPT_DIR"
cargo run --bin generate-test-repo -p test-utils -- --output "$OUTPUT_DIR" --verbose

echo "Test repository generated successfully!"
echo "To explore the repository:"
echo "  cd $OUTPUT_DIR"
echo "  git log --oneline --graph --all"