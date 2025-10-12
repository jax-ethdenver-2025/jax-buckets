#!/bin/bash

# Helper script for inspecting JAX node databases

if [ -z "$1" ]; then
    echo "Usage: $0 <node1|node2> [sql_query]"
    echo ""
    echo "Examples:"
    echo "  $0 node1                          # Open interactive SQLite shell"
    echo "  $0 node1 '.tables'                # List all tables"
    echo "  $0 node1 'SELECT * FROM buckets'  # Run a query"
    echo "  $0 node1 'SELECT * FROM bucket_shares'  # View shares"
    echo "  $0 node1 'SELECT * FROM bucket_peers'   # View peers"
    exit 1
fi

NODE=$1
QUERY=$2

if [ "$NODE" == "node1" ]; then
    DB_PATH="./data/node1/db.sqlite"
elif [ "$NODE" == "node2" ]; then
    DB_PATH="./data/node2/db.sqlite"
else
    echo "Error: Node must be 'node1' or 'node2'"
    exit 1
fi

if [ ! -f "$DB_PATH" ]; then
    echo "Error: Database not found at $DB_PATH"
    echo "Make sure you've run ./dev.sh first"
    exit 1
fi

if [ -z "$QUERY" ]; then
    # Interactive mode
    sqlite3 "$DB_PATH"
else
    # Execute query
    sqlite3 "$DB_PATH" "$QUERY"
fi
