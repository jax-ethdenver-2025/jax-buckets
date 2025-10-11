#!/usr/bin/env bash
set -e

# Navigate to rust directory
cd "$(dirname "$0")/.."

echo "Running Rust quality checks..."
echo ""

echo "==> Checking code formatting..."
cargo fmt --all -- --check

echo ""
echo "==> Running clippy..."
cargo clippy --workspace --all-targets --all-features -- -D warnings

echo ""
echo "==> Type checking with cargo check..."
cargo check --workspace --all-targets --all-features

echo ""
echo "All quality checks passed! ✅"
