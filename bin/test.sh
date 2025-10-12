#!/usr/bin/env bash
set -e

# Navigate to rust directory
cd "$(dirname "$0")/.."

echo "Running Rust tests..."
cargo test --workspace --all-features

echo ""
echo "All tests passed! ✅"
