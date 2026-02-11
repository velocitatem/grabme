#!/bin/bash
set -e

# Check dependencies
if ! command -v xvfb-run &> /dev/null; then
    echo "Error: xvfb-run is not installed. Please install 'xvfb' package."
    exit 1
fi

if ! command -v xdotool &> /dev/null; then
    echo "Error: xdotool is not installed. Please install 'xdotool' package."
    exit 1
fi

# Build the binary
echo "Building test binary..."
cargo build -p e2e-cursor-drift

# Output directory
OUTPUT_DIR="drift_test_output"
rm -rf "$OUTPUT_DIR"

echo "Running E2E Drift Test inside Xvfb..."
# Use 1280x720x24 resolution
# Force X11 polling backend to ensure xdotool movements are captured in Xvfb
export GRABME_FORCE_INPUT_BACKEND=x11
xvfb-run --server-args="-screen 0 1280x720x24" \
    cargo run -p e2e-cursor-drift -- --output-dir "$OUTPUT_DIR" --width 1280 --height 720 --dwell-time 1.0

echo "Test completed."
