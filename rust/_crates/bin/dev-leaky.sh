#!/usr/bin/env bash
set -euo pipefail
# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color
# Function to print usage
print_usage() {
	echo "Usage: $0 [command]"
	echo "Commands:"
	echo "  up        Start all services"
	echo "  down      Stop all services"
	echo "  restart   Restart all services"
	echo "  reset     Reset all services and data"
	echo "  reset-server Reset server and data"
	echo "  reset-data Reset data"
	echo "  logs      View logs of all services"
	echo "  ps        List running services"
	echo "  shell     Open a shell in a service container"
	echo "  test      Run integration tests"
}
# Function to start leaky server in a new tmux session
start_leaky_server() {
	tmux new-session -d -s leaky_server 'IPFS_RPC_URL=http://localhost:5001 GET_CONTENT_FORWARDING_URL=http://localhost:3001 cargo watch -x "run --bin leaky-server"'
	echo -e "${GREEN}Leaky server started in a new tmux session.${NC}"
	echo "To attach to the session, use: tmux attach-session -t leaky_server"
}
# Function to stop leaky server tmux session
stop_leaky_server() {
	if tmux has-session -t leaky_server 2>/dev/null; then
		# Send SIGTERM to the process running in the tmux session
		tmux send-keys -t leaky_server C-c

		# Wait for a moment to allow the process to gracefully shutdown
		sleep 2

		# Check if the process is still running
		if tmux list-panes -t leaky_server -F '#{pane_pid}' | xargs ps -p >/dev/null 2>&1; then
			echo -e "${YELLOW}Process didn't stop gracefully. Forcing termination...${NC}"
			# Force kill the process
			tmux list-panes -t leaky_server -F '#{pane_pid}' | xargs kill -9
		fi

		# Kill the tmux session
		tmux kill-session -t leaky_server
		echo -e "${GREEN}Leaky server stopped and tmux session killed.${NC}"
	else
		echo -e "${RED}Leaky server tmux session not found.${NC}"
	fi
}
# Check if Docker is running
if ! docker info >/dev/null 2>&1; then
	echo -e "${RED}Error: Docker is not running.${NC}"
	exit 1
fi
# Function to determine docker compose command
get_docker_compose_cmd() {
	if command -v docker-compose &>/dev/null; then
		echo "docker-compose"
	else
		echo "docker compose"
	fi
}
# Store the appropriate docker compose command
DOCKER_COMPOSE_CMD=$(get_docker_compose_cmd)
# Main script logic
case ${1:-} in
up)
	# Ensure data directories exist
	mkdir -p ./data/test
	echo -e "${GREEN}Starting services...${NC}"
	$DOCKER_COMPOSE_CMD up -d --build --remove-orphans
	;;
up-server)
	start_leaky_server
	;;
up-all)
	./bin/dev.sh up
	./bin/dev.sh up-server
	;;
down)
	echo -e "${GREEN}Stopping services...${NC}"
	$DOCKER_COMPOSE_CMD down
	;;
down-server)
	stop_leaky_server
	;;
down-all)
	./bin/dev.sh down
	./bin/dev.sh down-server
	;;
reset)
	./bin/dev.sh down
	docker volume rm leaky_ipfs_data || true
	./bin/dev.sh up
	;;
reset-fixtures)
	rm -rf data/pems
	rm -rf data/test
	mkdir -p ./data/pems
	cp -r ./crates/integration-tests/tests/fixtures ./data/test
	cd ./data/test
	cargo run --bin leaky-cli -- init --remote http://localhost:3001 --key-path ../pems &&
		cargo run --bin leaky-cli -- add &&
		cargo run --bin leaky-cli -- push
	;;
reset-server)
	./bin/dev.sh down-server
	rm -rf ./data/*db
	rm -rf ./data/*db*
	./bin/dev.sh up-server
	;;
reset-all)
	./bin/dev.sh reset
	./bin/dev.sh reset-server
	sleep 3
	./bin/dev.sh reset-fixtures
	;;
logs)
	echo -e "${GREEN}Viewing logs...${NC}"
	$DOCKER_COMPOSE_CMD logs -f
	;;
ps)
	echo -e "${GREEN}Listing running services...${NC}"
	$DOCKER_COMPOSE_CMD ps
	;;
shell)
	if [ -z ${2:-} ]; then
		echo -e "${RED}Error: Please specify a service name.${NC}"
		echo "Available services: ipfs, nginx"
		exit 1
	fi
	echo -e "${GREEN}Opening shell in $2 service...${NC}"
	$DOCKER_COMPOSE_CMD exec $2 /bin/sh
	;;
test)
	echo -e "${GREEN}Running integration tests...${NC}"
	# Start services in test mode
	$DOCKER_COMPOSE_CMD up -d --build
	start_leaky_server
	
	# Run the tests
	cd crates/leaky-cli && cargo test --test integration_test -- --test-threads=1
	
	# Cleanup
	./bin/dev.sh down
	;;
*)
	print_usage
	exit 1
	;;
esac
