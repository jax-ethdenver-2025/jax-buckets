#!/usr/bin/env bash

set -o errexit

export SQLITE_DATABASE_URL=$(bin/sqlite.sh database-url)

cargo run $@
