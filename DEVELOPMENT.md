# Development Guide

This guide covers setting up a development environment and working on JaxBucket.

## Prerequisites

### Required Tools

- **Rust**: 1.75+ (install via [rustup](https://rustup.rs/))
- **Cargo**: Comes with Rust
- **Git**: For version control
- **tmux**: For development environment (optional but recommended)
- **cargo-watch**: For auto-reload during development

### Install Development Tools

```bash
# Install Rust via rustup (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install cargo-watch for auto-reload
cargo install cargo-watch

# Install tmux (if not already installed)
# macOS
brew install tmux

# Linux (Ubuntu/Debian)
sudo apt install tmux

# Linux (Fedora/RHEL)
sudo dnf install tmux
```

### System Libraries

See [INSTALL.md](INSTALL.md) for required system libraries (OpenSSL, SQLite, etc.).

## Development Setup

### Clone the Repository

```bash
git clone https://github.com/jax-ethdenver-2025/jax-bucket.git
cd jax-bucket
```

### Build the Project

```bash
# Build in debug mode (faster compilation, slower runtime)
cargo build

# Build in release mode (slower compilation, faster runtime)
cargo build --release
```

### Verify Setup

```bash
# Run tests to ensure everything works
cargo test

# Check for compilation errors
cargo check

# Run the CLI
cargo run --bin jax -- --help
```

## Running the Development Environment

JaxBucket includes a convenient development script that sets up a **two-node P2P network** in tmux with auto-reload.

### Using `bin/dev.sh`

The `dev.sh` script creates a tmux session with:
- **Two JaxBucket nodes** running in parallel (Node1 and Node2)
- **Auto-reload** on code changes (via cargo-watch)
- **Separate windows** for database inspection and API testing

**Start the development environment:**

```bash
./bin/dev.sh
```

This will:
1. Initialize two nodes in `./data/node1` and `./data/node2` (if not already done)
2. Create a tmux session named `jax-dev`
3. Start both nodes with auto-reload
4. Attach to the tmux session

### Tmux Session Layout

The `jax-dev` session has three windows:

#### Window 0: `jax-nodes`
- **Left pane**: Node1 running with auto-reload
  - API: `http://localhost:3000`
  - Web UI: `http://localhost:8080`
  - P2P Port: 9000
- **Right pane**: Node2 running with auto-reload
  - API: `http://localhost:3001`
  - Web UI: `http://localhost:8081`
  - P2P Port: 9005

Both nodes automatically restart when you save code changes.

#### Window 1: `db`
Provides database inspection information:
- Node1 DB: `./data/node1/db.sqlite`
- Node2 DB: `./data/node2/db.sqlite`

Use the `./bin/db.sh` script to inspect databases:
```bash
./bin/db.sh node1  # Open Node1 database
./bin/db.sh node2  # Open Node2 database
```

#### Window 2: `api`
API testing window with endpoint information:
```bash
# Node1 endpoints
curl http://localhost:3000/api/buckets
curl http://localhost:3000/api/node/info

# Node2 endpoints
curl http://localhost:3001/api/buckets
curl http://localhost:3001/api/node/info

# Open web UIs
open http://localhost:8080  # Node1 UI
open http://localhost:8081  # Node2 UI
```

### Tmux Navigation

Basic tmux commands:

```bash
# Switch between windows
Ctrl+b 0  # Go to window 0 (nodes)
Ctrl+b 1  # Go to window 1 (db)
Ctrl+b 2  # Go to window 2 (api)

# Switch between panes (within window 0)
Ctrl+b Left/Right Arrow

# Scroll in pane
Ctrl+b [  # Enter scroll mode
# Use arrow keys or Page Up/Down
# Press q to exit scroll mode

# Detach from session (keeps running)
Ctrl+b d

# Reattach later
tmux attach -t jax-dev

# Kill the entire session
tmux kill-session -t jax-dev
```

### Testing P2P Sync

With both nodes running:

1. **Get Node IDs:**
   ```bash
   # In window 2 (api)
   curl http://localhost:3000/api/node/info | jq .node_id
   curl http://localhost:3001/api/node/info | jq .node_id
   ```

2. **Create a bucket on Node1:**
   ```bash
   curl -X POST http://localhost:3000/api/buckets \
     -H "Content-Type: application/json" \
     -d '{"name": "test-bucket"}'
   ```

3. **Share with Node2:**
   ```bash
   curl -X POST http://localhost:3000/api/buckets/{bucket-id}/share \
     -H "Content-Type: application/json" \
     -d '{"peer_id": "NODE2_ID", "role": "editor"}'
   ```

4. **Verify:**
   ```bash
   curl http://localhost:3001/api/buckets
   ```

## Project Structure

```text
jax-bucket/
├── Cargo.toml                 # Workspace configuration
├── bin/
│   ├── dev.sh                 # Development environment script
│   └── db.sh                  # Database inspection script
├── crates/
│   ├── common/                # Core library (platform-agnostic)
│   │   ├── src/
│   │   │   ├── bucket/        # Bucket, Manifest, Node, Pins
│   │   │   ├── crypto/        # Keys, Secrets, Shares
│   │   │   ├── linked_data/   # Link, CID, DAG-CBOR
│   │   │   └── peer/          # Peer, BlobsStore, JAX Protocol
│   │   ├── Cargo.toml
│   │   └── CHANGELOG.md
│   ├── service/               # Server-side logic
│   │   ├── src/
│   │   │   ├── database/      # SQLite models and queries
│   │   │   ├── http_server/   # API and Web UI
│   │   │   ├── mount_ops/     # Bucket operations
│   │   │   └── sync_manager/  # P2P sync logic
│   │   ├── Cargo.toml
│   │   └── CHANGELOG.md
│   └── app/                   # CLI binary
│       ├── src/
│       │   ├── main.rs        # Entry point
│       │   └── ops/           # CLI commands
│       ├── Cargo.toml
│       └── CHANGELOG.md
├── README.md                  # Project overview
├── INSTALL.md                 # Installation guide
├── PROTOCOL.md                # Protocol specification
├── DEVELOPMENT.md             # This file
└── CONTRIBUTING.md            # Contribution guidelines
```

### Key Crates

#### `common` - Core Library

Platform-agnostic data structures and cryptography:

- **`bucket/`**: Core data model
  - `manifest.rs` - Bucket manifests
  - `node.rs` - Directory nodes
  - `mount.rs` - High-level bucket operations
  - `pins.rs` - Content pinning

- **`crypto/`**: Cryptographic primitives
  - `keys.rs` - Ed25519 identity keys
  - `secret.rs` - ChaCha20-Poly1305 encryption
  - `share.rs` - ECDH + AES-KW key sharing

- **`linked_data/`**: Content addressing
  - `link.rs` - CID links
  - `codec.rs` - IPLD codecs

- **`peer/`**: P2P networking
  - `mod.rs` - Peer and BlobsStore
  - `jax_protocol/` - Custom sync protocol

#### `service` - HTTP Server & Sync

Server-side components:

- **`database/`**: SQLite persistence
- **`http_server/`**: REST API and Web UI
- **`mount_ops/`**: Bucket CRUD operations
- **`sync_manager/`**: Background P2P sync

#### `app` - CLI Binary

Command-line interface:

- **`ops/`**: CLI commands (init, service, bucket)
- **`main.rs`**: Argument parsing and dispatch

## Testing

### Run All Tests

```bash
cargo test
```

### Run Tests for a Specific Crate

```bash
cargo test -p common
cargo test -p service
cargo test -p jax-bucket
```

### Run a Specific Test

```bash
cargo test test_name
```

### Run Tests with Logging

```bash
RUST_LOG=debug cargo test
```

### Run Tests with Output

```bash
cargo test -- --nocapture
```

### Integration Tests

Integration tests are located in `crates/*/tests/`:

```bash
# Run only integration tests
cargo test --test '*'
### In-Process Integration Tests (testkit)

Enable the `testkit` feature to run in-process multi-peer tests without external binaries.

Run tests:

```bash
cargo test -p jax-service --features testkit -- --nocapture
```

Example scenario: share with a peer, take them offline, add files, bring them back and pull.

```rust
// crates/service/tests/share_offline_then_sync.rs
#![cfg(feature = "testkit")]

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn share_offline_then_sync() -> anyhow::Result<()> {
    use std::time::Duration;
    use service::testkit::*;

    let mut net = TestNetwork::new();
    let alice = net.add_peer("alice");
    let bob = net.add_peer("bob");

    alice.start().await?;
    bob.start().await?;

    let bucket = alice.create_bucket("shared").await?;
    alice.share_bucket_with(bucket, bob).await?;

    bob.stop().await?;
    alice.add_file_bytes(bucket, "/a.txt", b"hello").await?;
    alice.add_file_bytes(bucket, "/b.txt", b"world").await?;
    bob.start().await?;
    bob.trigger_pull(bucket).await?;

    net.eventually(Duration::from_secs(15), || async {
        Ok(bob.has_file(bucket, "/a.txt").await?)
    }).await?;
    net.eventually(Duration::from_secs(15), || async {
        Ok(bob.has_file(bucket, "/b.txt").await?)
    }).await?;

    Ok(())
}
```

```

## Code Style

### Formatting

Use `rustfmt` to format code:

```bash
# Format all code
cargo fmt

# Check formatting without making changes
cargo fmt -- --check
```

### Linting

Use `clippy` for linting:

```bash
# Run clippy
cargo clippy

# Run clippy with warnings as errors
cargo clippy -- -D warnings

# Apply automatic fixes
cargo clippy --fix
```

### Code Conventions

- **Naming**: Use snake_case for functions/variables, PascalCase for types
- **Documentation**: Document all public APIs with `///` comments
- **Error Handling**: Use `anyhow::Result` for application errors, custom types for library errors
- **Imports**: Group std, external crates, and local imports separately
- **Line Length**: Target 100 characters, but not a hard limit

Example:
```rust
/// Creates a new encrypted bucket with the given name.
///
/// # Arguments
///
/// * `name` - Human-readable name for the bucket
/// * `secret` - Encryption secret for the root node
///
/// # Returns
///
/// The newly created manifest
pub fn create_bucket(name: String, secret: Secret) -> anyhow::Result<Manifest> {
    // Implementation
}
```

## Debugging

### Enable Debug Logging

```bash
RUST_LOG=debug cargo run --bin jax -- service
```

### Logging Levels

- `error` - Critical errors only
- `warn` - Warnings and errors
- `info` - General information (default)
- `debug` - Detailed debugging
- `trace` - Very verbose tracing

### Filter by Module

```bash
# Only show logs from sync_manager
RUST_LOG=service::sync_manager=debug cargo run --bin jax -- service

# Multiple modules
RUST_LOG=service::sync_manager=debug,common::peer=trace cargo run --bin jax -- service
```

### Inspect Database

```bash
# Open database with sqlite3
sqlite3 ~/.config/jax/jax.db

# Or use the dev script
./bin/db.sh node1
```

### Inspect Blobs

Blobs are stored in `~/.config/jax/blobs/`. You can inspect them with Iroh tools or directly:

```bash
# List blobs
ls -lh ~/.config/jax/blobs/

# View blob metadata (they're encrypted, so you'll see ciphertext)
hexdump -C ~/.config/jax/blobs/HASH
```

## Common Development Tasks

### Add a New CLI Command

1. Create a new module in `crates/app/src/ops/`
2. Implement the command logic
3. Add the command to `crates/app/src/main.rs`
4. Add tests

### Add a New API Endpoint

1. Add route in `crates/service/src/http_server/mod.rs`
2. Implement handler function
3. Update API documentation
4. Add integration test

### Add a New Sync Message Type

1. Update `crates/common/src/peer/jax_protocol/messages.rs`
2. Implement serialization/deserialization
3. Add handler in `crates/service/src/sync_manager/mod.rs`
4. Update protocol documentation

### Modify the Data Model

1. Update structs in `crates/common/src/bucket/`
2. Add migration logic if needed
3. Update serialization tests
4. Run tests to ensure compatibility

## Resources

- **Rust Book**: https://doc.rust-lang.org/book/
- **Cargo Book**: https://doc.rust-lang.org/cargo/
- **Iroh Documentation**: https://docs.iroh.computer/
- **IPLD Specs**: https://ipld.io/specs/

## Getting Help

- **Issues**: https://github.com/jax-ethdenver-2025/jax-bucket/issues
- **Discussions**: https://github.com/jax-ethdenver-2025/jax-bucket/discussions

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on contributing to JaxBucket.
