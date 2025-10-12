#!/bin/bash
set -e

# Check if config path is initialized
if [ ! -d "${CONFIG_PATH}" ] || [ ! -f "${CONFIG_PATH}/config.toml" ]; then
    echo "Initializing JAX node at ${CONFIG_PATH}..."
    jax --config-path "${CONFIG_PATH}" init \
        --api-addr "${API_ADDR}" \
        --html-addr "${HTML_ADDR}" \
        --peer-port "${PEER_PORT}"
    echo "Node initialized successfully"
else
    echo "Using existing configuration at ${CONFIG_PATH}"
fi

# Start the service
echo "Starting JAX service..."
exec jax --config-path "${CONFIG_PATH}" "$@"
