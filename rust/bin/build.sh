#!/usr/bin/env bash
set -e

# Navigate to rust directory
cd "$(dirname "$0")/.."

echo "Building Rust project..."
cargo build --workspace --all-features --release

echo ""
echo "Build complete! ✅"
