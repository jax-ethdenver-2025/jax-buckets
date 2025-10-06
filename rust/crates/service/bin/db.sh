#!/usr/bin/env bash

set -o errexit
set -o nounset

DEFAULT_DATABASE_URL="sqlite://$(pwd)/data/node.db"

function database-url {
	# Check if the DATABASE_URL environment variable is set
	# If it is, use that
	if [ -n "${DATABASE_URL:-}" ]; then
		echo ${DATABASE_URL}
		return
	fi
	echo ${DEFAULT_DATABASE_URL}
}

function create() {
	# Get the database URL
	local database_url=$(database-url)
	# Check if its a path
	if [[ ${database_url} == sqlite://* ]]; then
		# Get the path
		local path=${database_url#sqlite://}
		# Create the directory
		mkdir -p $(dirname ${path})
	fi
	# Use sqlx to create the database
	cargo sqlx database create --database-url ${database_url}
}

function queries() {
	# Get the database URL
	local database_url=$(database-url)
	# Use sqlx to run the queries
	cargo sqlx prepare --database-url ${database_url} -- --all-targets --all-features --tests
}

function migrate() {
	# Get the database URL
	local database_url=$(database-url)
	# Use sqlx to run the migrations
	cargo sqlx migrate run --database-url ${database_url}
}

function clean() {
	# Get the database URL
	local database_url=$(database-url)
	# Check if its a path
	if [[ ${database_url} == sqlite://* ]]; then
		# Get the path
		local path=${database_url#sqlite://}
		# Remove the file
		rm -f ${path}
	fi
}

$1
