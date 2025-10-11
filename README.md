# JaxBucket

**End-to-End Encrypted Storage Buckets with Peer-to-Peer Synchronization**

JaxBucket is a local-first, encrypted storage system built on [Iroh](https://iroh.computer/). It provides content-addressed, encrypted file storage with automatic peer-to-peer synchronization between authorized devices.

## Features

- 🔒 **End-to-End Encryption**: All files encrypted with ChaCha20-Poly1305 AEAD
- 🌐 **P2P Sync**: Automatic synchronization via Iroh's networking stack
- 📦 **Content-Addressed**: Files and directories stored as immutable, hash-linked DAGs
- 🔑 **Cryptographic Key Sharing**: ECDH + AES Key Wrap for secure multi-device access
- 🌳 **Merkle DAG Structure**: Efficient verification and deduplication
- 🎯 **Local-First**: Works offline, syncs when connected
- 📌 **Selective Pinning**: Control which content to keep locally
- 🌍 **DHT Discovery**: Find peers via distributed hash table

## Architecture

### Overview

JaxBucket combines three key technologies:

1. **Iroh**: Provides the networking layer (QUIC, NAT traversal, DHT discovery)
2. **Content Addressing**: Files and directories are stored as BLAKE3-hashed blobs
3. **Encryption**: Each node/file has its own encryption key, shared via ECDH

```text
┌─────────────────────────────────────────────────┐
│                   JaxBucket                      │
├─────────────────────────────────────────────────┤
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐  │
│  │ Buckets  │  │  Crypto  │  │ Sync Manager │  │
│  │  (DAG)   │  │(ECDH+AES)│  │(Pull/Push)   │  │
│  └────┬─────┘  └─────┬────┘  └──────┬───────┘  │
├───────┼──────────────┼───────────────┼──────────┤
│  ┌────▼──────────────▼───────────────▼───────┐  │
│  │        Iroh Networking Layer              │  │
│  │  (QUIC + DHT Discovery + BlobStore)       │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

### Data Model

#### Buckets

A **bucket** is a versioned collection of encrypted files and directories. Each bucket has:

- **Unique ID**: UUID for global identification
- **Name**: Human-readable label (not unique)
- **Manifest**: Unencrypted metadata block containing:
  - Entry Link: Points to the root directory node
  - Shares: Map of `PublicKey -> BucketShare` for access control
  - Pins: Link to the pinset (content to keep locally)
  - Previous: Link to the prior version (forms a version chain)
  - Version: Software version metadata

#### Nodes

A **node** represents a directory in the bucket's file tree. Nodes are:

- **Encrypted**: Each node is encrypted with its own secret key
- **DAG-CBOR Encoded**: Serialized using DAG-CBOR before encryption
- **Content-Addressed**: Hashed after encryption for stable addressing

Node structure:
```rust
Node {
    links: BTreeMap<String, NodeLink>
}

NodeLink::Data(link, secret, metadata)  // File
NodeLink::Dir(link, secret)              // Subdirectory
```

Each `NodeLink` contains:
- **Link**: Content-addressed pointer (codec + hash + format)
- **Secret**: Encryption key for decrypting the target
- **Metadata** (for files): MIME type, custom properties

#### Manifest Deep Dive

The `Manifest` type (rust/crates/common/src/bucket/manifest.rs:54) stores bucket metadata:

```rust
Manifest {
    id: Uuid,                    // Global bucket identifier
    name: String,                // Display name
    shares: Shares,              // Access control: PublicKey -> BucketShare
    entry: Link,                 // Root directory node
    pins: Link,                  // HashSeq of pinned content
    previous: Option<Link>,      // Previous manifest version
    version: Version,            // Software version
}
```

**BucketShare** wraps an encryption secret for a specific peer:
```rust
BucketShare {
    principal: Principal {       // Peer identity + role
        role: PrincipalRole,     // Owner, Editor, Viewer
        identity: PublicKey,     // Peer's public key
    },
    share: Share,                // ECDH-wrapped bucket secret
}
```

### Peer Structure

A **peer** (rust/crates/common/src/peer/mod.rs:129) represents a JaxBucket node on the network:

```rust
Peer {
    blob_store: BlobsStore,      // Content storage (Iroh blobs)
    secret: SecretKey,           // Ed25519 identity keypair
    endpoint: Endpoint,          // Iroh QUIC endpoint
    protocol_state: BucketStateProvider, // Access to local bucket state
}
```

#### Peer Components

1. **Identity**: Ed25519 keypair (SecretKey/PublicKey)
   - Public key = NodeId for Iroh networking
   - Same key used for ECDH key sharing (converted to X25519)

2. **BlobsStore**: Iroh's content-addressed blob storage
   - Stores encrypted nodes and files
   - Deduplicates by hash
   - Supports both Raw blobs and HashSeq (collections)

3. **Endpoint**: Iroh's QUIC networking
   - NAT traversal via STUN/TURN
   - DHT-based peer discovery (Mainline DHT)
   - Custom ALPN protocols (iroh-blobs + jax-protocol)

4. **Protocol State**: Interface to local database
   - Queries bucket information
   - Tracks sync status
   - Handles incoming announcements

### Cryptography

#### Identity & Key Sharing

Each peer has an **Ed25519 keypair**:
- **SecretKey**: Stored locally (e.g., `~/.config/jax/secret.pem`)
- **PublicKey**: Used as peer ID and for key sharing

To share a bucket with another peer:

1. **Generate Ephemeral Key**: Create temporary Ed25519 keypair
2. **ECDH**: Convert to X25519 and compute shared secret
3. **AES Key Wrap**: Wrap bucket secret with shared secret (RFC 3394)
4. **Share**: Package as `[ephemeral_pubkey(32) || wrapped_secret(40)]`

The recipient recovers the secret by:
1. Extract ephemeral public key from Share
2. Perform ECDH with their private key
3. Use AES-KW to unwrap the bucket secret

See `rust/crates/common/src/crypto/share.rs` for implementation.

#### Content Encryption

Files and nodes are encrypted with **ChaCha20-Poly1305**:

- **Per-Item Keys**: Each file/node has its own 256-bit secret
- **AEAD**: Authenticated encryption with additional data
- **Format**: `nonce(12) || ciphertext || tag(16)`

This provides:
- **Content-Addressed Storage**: Hashes are stable after encryption
- **Fine-Grained Access**: Can selectively share keys
- **Efficient Updates**: Only re-encrypt changed items

See `rust/crates/common/src/crypto/secret.rs` for implementation.

### Synchronization Protocol

JaxBucket implements a custom sync protocol (JAX Protocol) on top of Iroh:

#### Protocol Messages

The protocol (rust/crates/common/src/peer/jax_protocol/messages.rs) defines three operations:

1. **Ping**: Check sync status of a bucket
   ```rust
   PingRequest { bucket_id, current_link }
   → PingResponse { status: NotFound | Behind | InSync | Ahead }
   ```

2. **Fetch**: Retrieve current bucket link
   ```rust
   FetchBucketRequest { bucket_id }
   → FetchBucketResponse { current_link: Option<Link> }
   ```

3. **Announce**: Notify peers of new version (fire-and-forget)
   ```rust
   AnnounceMessage { bucket_id, new_link, previous_link }
   ```

#### Sync Workflow

**Pull Sync** (rust/crates/service/src/sync_manager/mod.rs:204):

1. Ping all peers in parallel for the bucket
2. Find a peer reporting `Ahead` status
3. Fetch the new bucket link from that peer
4. Download the manifest blob via Iroh blobs protocol
5. Verify **single-hop**: peer's previous must equal our current
6. Download the pinset (HashSeq of required content)
7. Update database with new link, mark as Synced

**Push Sync** (rust/crates/service/src/sync_manager/mod.rs:420):

1. Get list of peers for the bucket
2. Read the new manifest to extract previous link
3. Send announce messages to all peers in parallel
4. Log results (best-effort delivery)

**Peer Announce Handler** (rust/crates/service/src/sync_manager/mod.rs:483):

1. Receive announce from peer
2. Verify **provenance**: peer must be in bucket shares
3. Verify **single-hop**: previous must equal our current
4. Download manifest and pinset from announcing peer
5. Update database with new link

#### Sync Verification

Two safety checks prevent invalid updates:

1. **Provenance**: Only peers in the bucket's `shares` can send announces
2. **Single-Hop**: Updates must reference our current link as `previous`
   - Prevents accepting stale or forked versions
   - Ensures linear version history
   - If check fails, triggers full pull sync to reconcile

### Pinning

**Pins** (rust/crates/common/src/bucket/pins.rs) are a set of content hashes that should be kept locally:

```rust
Pins(HashSet<Hash>)  // Set of BLAKE3 hashes
```

When a bucket is saved:
1. All node hashes are added to pins
2. Pins are serialized as a **HashSeq** (Iroh's hash list format)
3. HashSeq is stored as a blob
4. Manifest points to the pins HashSeq

When syncing:
1. Download the pins HashSeq from peer
2. Verify all pinned hashes are available locally
3. Iroh automatically fetches missing blobs

## Usage

### Service Launch

The JaxBucket service provides an HTTP API and background sync manager.

#### Initialize Configuration

```bash
# Create config directory and generate identity
jax init
```

This creates `~/.config/jax/` with:
- `config.toml`: Service configuration
- `secret.pem`: Ed25519 identity keypair
- `jax.db`: SQLite database for bucket metadata

#### Start Service

```bash
# Start service (default: http://localhost:8080)
jax service

# Custom port
jax service

# Custom config path
jax service --config /path/to/config.toml
```

The service runs:
- **HTTP Server**: REST API + Web UI
- **Sync Manager**: Background task processing sync events
- **Iroh Peer**: QUIC endpoint for P2P networking

#### Service Configuration

Edit `~/.config/jax/config.toml`:

```toml
[node]
secret_key_path = "~/.config/jax/secret.pem"
blobs_path = "~/.config/jax/blobs"
bind_port = 0  # 0 = ephemeral port

[database]
path = "~/.config/jax/jax.db"

[http_server]
host = "127.0.0.1"
port = 8080
```

### Web UI

Navigate to `http://localhost:8080` to access the web interface:

- **Dashboard** (`/`): Overview of buckets and sync status
- **Bucket Explorer** (`/bucket/{id}`): Browse bucket contents
- **File Viewer** (`/bucket/{id}/file/{path}`): View/download files
- **Pins Explorer** (`/pins/{id}`): View pinned content
- **Peers Explorer** (`/peers`): View connected peers

The UI is built with server-side HTML rendering (no JavaScript framework).

## Installation

### From Source

```bash
# Clone repository
git clone https://github.com/jax-ethdenver-2025/jax-bucket
cd jax-bucket

# Build all crates
cargo build --release

# Install binaries
cargo install --path rust/crates/app
```

Binaries will be installed to `~/.cargo/bin/jax`.

### Requirements

- **Rust**: 1.75+ (2021 edition)
- **System Libraries**: OpenSSL, libsqlite3
- **OS**: Linux, macOS, Windows (WSL2 recommended)

## Project Structure

```text
jax-bucket/
├── rust/
│   └── crates/
│       ├── common/          # Core data structures and crypto
│       │   ├── bucket/      # Manifest, Node, Mount, Pins
│       │   ├── crypto/      # Keys, Secrets, Shares
│       │   ├── linked_data/ # Link, CID, DAG-CBOR
│       │   └── peer/        # Peer, BlobsStore, JAX Protocol
│       ├── service/         # HTTP server and sync manager
│       │   ├── database/    # SQLite models
│       │   ├── http_server/ # API and web UI
│       │   ├── mount_ops/   # Bucket operations
│       │   └── sync_manager # P2P sync logic
│       └── app/             # CLI binary
│           └── ops/         # CLI commands
├── README.md
└── Cargo.toml
```

### Key Files

- **Manifest**: `rust/crates/common/src/bucket/manifest.rs`
- **Node**: `rust/crates/common/src/bucket/node.rs`
- **Mount**: `rust/crates/common/src/bucket/mount.rs`
- **Crypto**: `rust/crates/common/src/crypto/`
- **Peer**: `rust/crates/common/src/peer/mod.rs`
- **JAX Protocol**: `rust/crates/common/src/peer/jax_protocol/`
- **Sync Manager**: `rust/crates/service/src/sync_manager/mod.rs`
- **CLI**: `rust/crates/app/src/ops/`

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p common
cargo test -p service

# Run with logging
RUST_LOG=debug cargo test
```

### Running the Service Locally

```bash
# Terminal 1: Run service
RUST_LOG=info cargo run --bin jax -- service start

# Terminal 2: Use CLI
cargo run --bin jax -- bucket create test-bucket
cargo run --bin jax -- bucket list
```

### Code Organization

- **`common`**: Platform-agnostic core (can be used in WASM)
- **`service`**: Server-side logic (HTTP, database, sync)
- **`app`**: CLI binary

## Security Considerations

### Threat Model

JaxBucket protects against:

- ✅ **Untrusted Storage**: Blobs are encrypted, storage provider sees only hashes
- ✅ **Passive Network Observers**: All peer connections use QUIC + TLS
- ✅ **Unauthorized Peers**: Only peers with valid `BucketShare` can decrypt content
- ✅ **Tampered Data**: AEAD and content addressing detect modifications

JaxBucket does NOT protect against:

- ❌ **Compromised Peer**: If an attacker gains access to your secret key or config
- ❌ **Malicious Authorized Peer**: Peers with valid shares can leak data
- ❌ **Metadata Leakage**: Bucket structure (file count, sizes) visible to storage provider
- ❌ **Traffic Analysis**: Connection patterns may reveal peer relationships

### Best Practices

1. **Protect Secret Keys**: Store `secret.pem` securely, use file permissions
2. **Verify Peer Identity**: Check public key fingerprints before sharing buckets
3. **Regular Key Rotation**: Periodically rotate bucket secrets
4. **Audit Shares**: Review who has access to your buckets
5. **Monitor Sync Status**: Check for unexpected updates

## Roadmap

- [ ] **Conflict Resolution**: Handle concurrent updates from multiple peers
- [ ] **Garbage Collection**: Remove unpinned blobs to free space
- [ ] **Streaming Encryption**: Efficient handling of large files
- [ ] **Access Revocation**: Remove peer access and re-encrypt
- [ ] **Mobile Support**: iOS and Android apps
- [ ] **WASM Support**: Run in web browsers
- [ ] **Selective Sync**: Only sync specific subdirectories
- [ ] **Compression**: Compress before encryption

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure `cargo test` passes
5. Submit a pull request

## Acknowledgments

Built with:
- **[Iroh](https://iroh.computer/)**: P2P networking and content storage
- **[Rust](https://www.rust-lang.org/)**: Systems programming language
- **[DAG-CBOR](https://ipld.io/)**: Merkle DAG serialization

## Contact

- **Issues**: https://github.com/jax-ethdenver-2025/jax-bucket/issues
- **Discussions**: https://github.com/jax-ethdenver-2025/jax-bucket/discussions
