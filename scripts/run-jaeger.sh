#!/bin/bash

# Script to run Jaeger v2 with OTLP support
# Automatically downloads Jaeger if not present

set -euo pipefail

# Get the script directory (project root/scripts)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Jaeger will be installed in .tools/jaeger
JAEGER_DIR="$PROJECT_ROOT/.tools/jaeger"
# Jaeger v2 uses 'jaeger' binary instead of 'jaeger-all-in-one'
JAEGER_BIN="$JAEGER_DIR/jaeger"

# Check if Jaeger is installed, if not, download it
if [ ! -f "$JAEGER_BIN" ]; then
    echo "🔍 Jaeger v2 not found, downloading..."
    echo ""
    
    # Run the install script with the .tools/jaeger directory
    "$SCRIPT_DIR/install-jaeger.sh" "$JAEGER_DIR"
    
    if [ ! -f "$JAEGER_BIN" ]; then
        echo "❌ Failed to install Jaeger v2"
        exit 1
    fi
fi

echo "🚀 Starting Jaeger v2 with built-in All-in-One configuration..."
echo ""
echo "📊 Jaeger UI will be available at: http://localhost:16686"
echo "📡 OTLP gRPC endpoint: localhost:4317"
echo "📡 OTLP HTTP endpoint: localhost:4318"
echo ""
echo "Press Ctrl+C to stop"
echo ""

# Run Jaeger v2 without config - uses built-in All-in-One configuration
"$JAEGER_BIN"
