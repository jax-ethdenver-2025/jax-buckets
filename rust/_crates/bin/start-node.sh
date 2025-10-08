#!/bin/bash
set -e

# Configuration
ANVIL_PORT=8545

# Exit if anvil is not running
if ! nc -z localhost "$ANVIL_PORT" >/dev/null 2>&1; then
  echo "Anvil is not running on port $ANVIL_PORT. Exiting."
  exit 1
fi

# Test account (from Anvil's default accounts)
NODE_KEYS=(
  "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
  "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
  "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a"
  "0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6"
  "0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a"
  "0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba"
  "0x92db14e403b83dfe3df233f83dfa3a0d7096f21ca9b0d6d6b8d88b2b4ec1564e"
  "0x4bbbf85ce3377467afe5d46f804f221813b2bb87f24d81f60f1fcdbf7cbf4356"
  "0xdbda1821b80551c9d65939329250298aa3472ba22feea921c0cf5d620ea67b97"
  "0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6"
)

# get the node index from the command line
NODE_INDEX=$1

NODE_KEY=${NODE_KEYS[$NODE_INDEX]}
NODE_REMOTE_PORT=$((8080 + NODE_INDEX))
NODE_ENDPOINT_PORT=$((3001 + NODE_INDEX))

# check if the overwrite flag is set
if [ "$2" = "--overwrite" ]; then
  OVERWRITE=true
else
  OVERWRITE=false
fi

export JAX_CONFIG_PATH="./data/jax-node$NODE_INDEX"

function init_node() {
 # this should source the factory address
  source .env.local

  # check if the factory address is set
  if [ -z "$FACTORY_ADDRESS" ]; then
    echo "FACTORY_ADDRESS is not set. Exiting."
    exit 1
  fi

  cargo run --bin jax -- init \
    --overwrite \
    --factory-address $FACTORY_ADDRESS \
    --eth-signer $NODE_KEY \
    --http-port $NODE_REMOTE_PORT \
    --iroh-port $NODE_ENDPOINT_PORT 
}

# check if the config path exists
if [ ! -d "$JAX_CONFIG_PATH" ]; then
  init_node
elif [ "$OVERWRITE" = true ]; then
  init_node
fi

alias jax="cargo run --bin jax -- "

cargo run --bin jax -- node
