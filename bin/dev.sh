#!/bin/bash

# Development script for running two JAX nodes in tmux with watch mode
# This sets up a tmux session with two panes, each running a node with auto-reload

set -e

# Get the script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Change to project root
cd "$PROJECT_ROOT"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}Setting up JAX development environment...${NC}"

# Initialize nodes if they haven't been initialized yet
init_node() {
    local node_dir=$1
    local node_name=$2
    local api_port=$3
    local html_port=$4
    local peer_port=$5

    if [ ! -d "$node_dir" ]; then
        echo -e "${YELLOW}Initializing $node_name (first run)...${NC}"
        cargo run --bin jax -- --config-path "$node_dir" init --api-addr "0.0.0.0:$api_port" --html-addr "0.0.0.0:$html_port" --peer-port "$peer_port"
        echo -e "${GREEN}$node_name initialized with API:$api_port HTML:$html_port PEER:$peer_port${NC}"
    else
        echo -e "${GREEN}$node_name already initialized${NC}"
    fi
}

# Initialize both nodes
init_node "./data/node1" "Node1" 3000 8080 9000
init_node "./data/node2" "Node2" 3001 8081 9005

# Check if tmux session already exists
if tmux has-session -t jax-dev 2>/dev/null; then
    echo -e "${BLUE}Killing existing jax-dev tmux session...${NC}"
    tmux kill-session -t jax-dev
fi

# Check if cargo-watch is installed
if ! command -v cargo-watch &>/dev/null; then
    echo -e "${BLUE}cargo-watch not found, installing...${NC}"
    cargo install cargo-watch
fi

echo -e "${GREEN}Starting tmux session 'jax-dev'...${NC}"

# Create new tmux session with first node
tmux new-session -d -s jax-dev -n "jax-nodes"

# Split window horizontally for second node
tmux split-window -h -t jax-dev:0

# Run node1 in left pane with cargo watch
tmux send-keys -t jax-dev:0.0 "cd $PROJECT_ROOT && RUST_LOG=info cargo watch -x 'run --bin jax -- --config-path ./data/node1 service'" C-m

# Run node2 in right pane with cargo watch
tmux send-keys -t jax-dev:0.1 "cd $PROJECT_ROOT && RUST_LOG=info cargo watch -x 'run --bin jax -- --config-path ./data/node2 service'" C-m

# Create a new window for database inspection
tmux new-window -t jax-dev:1 -n "db"
tmux send-keys -t jax-dev:1 "cd $PROJECT_ROOT && echo 'Node1 DB: ./data/node1/db.sqlite' && echo 'Node2 DB: ./data/node2/db.sqlite' && echo '' && echo 'Use: ./bin/db.sh node1 or ./bin/db.sh node2'" C-m

# Create a new window for API testing
tmux new-window -t jax-dev:2 -n "api"
tmux send-keys -t jax-dev:2 "cd $PROJECT_ROOT && echo 'Node1 API: http://localhost:3000' && echo 'Node2 API: http://localhost:3001' && echo 'Node1 UI: http://localhost:8080' && echo 'Node2 UI: http://localhost:8081'" C-m

# Go back to first window
tmux select-window -t jax-dev:0

echo -e "${GREEN}Tmux session 'jax-dev' started!${NC}"
echo ""
echo "Usage:"
echo "  tmux attach -t jax-dev    # Attach to the session"
echo "  tmux kill-session -t jax-dev    # Kill the session"
echo ""
echo "Windows:"
echo "  0: jax-nodes  - Node1 (left) and Node2 (right) running with auto-reload"
echo "  1: db         - Database inspection window"
echo "  2: api        - API testing window"
echo ""
echo "Ports:"
echo "  Node1: API=3000, UI=8080, PEER=9000"
echo "  Node2: API=3001, UI=8081, PEER=9001"
echo ""
echo -e "${BLUE}Attaching to session...${NC}"

# Attach to the session
tmux attach -t jax-dev
