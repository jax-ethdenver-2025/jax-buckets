#!/usr/bin/env bash

# Watch for changes and restart the service
# Usage: ./watch-service.sh

set -o errexit
set -o nounset

echo "Starting cargo watch for service..."
echo "Will restart on changes to .rs files in crates/service, crates/common, and crates/app"
echo ""

cargo watch \
    -w crates/service/src \
    -w crates/common/src \
    -w crates/app/src \
    -x 'run --bin cli -- service'
