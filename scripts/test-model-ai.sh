#!/bin/bash
# Script to run model-ai tests in release mode for performance testing

set -e

echo "🧪 Running model-ai tests in release mode..."
echo "Testing all enabled models with both files-only and full-diff modes"
echo "================================================"

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo "Error: Please run this script from the project root directory"
    exit 1
fi

# Make the script executable on first run
chmod +x "$0"

# Default to running all tests
TEST_FILTER="${1:-}"

if [ -z "$TEST_FILTER" ]; then
    echo "Running all model-ai tests..."
    cargo test -p model-ai --release
elif [ "$TEST_FILTER" == "all-models" ]; then
    echo "Running comprehensive all models/modes test..."
    cargo test -p model-ai --release test_all_models_all_modes -- --nocapture
elif [ "$TEST_FILTER" == "prompt" ]; then
    echo "Running prompt generation tests..."
    cargo test -p model-ai --release prompt
elif [ "$TEST_FILTER" == "files" ]; then
    echo "Running files-only tests..."
    cargo test -p model-ai --release files_only
else
    echo "Running tests matching: $TEST_FILTER"
    cargo test -p model-ai --release "$TEST_FILTER"
fi

echo ""
echo "✅ Tests completed!"
echo ""
echo "Usage:"
echo "  ./scripts/test-model-ai.sh                # Run all tests"
echo "  ./scripts/test-model-ai.sh all-models     # Run comprehensive all models/modes test"
echo "  ./scripts/test-model-ai.sh prompt         # Run prompt tests only"
echo "  ./scripts/test-model-ai.sh files          # Run files-only tests"
echo "  ./scripts/test-model-ai.sh <pattern>      # Run tests matching pattern"