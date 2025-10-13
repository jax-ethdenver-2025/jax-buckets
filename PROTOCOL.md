# JaxBucket Protocol Specification

This document describes the technical details of the JaxBucket protocol, including data structures, cryptography, and synchronization mechanisms.

## Table of Contents

- [Overview](#overview)
- [Data Model](#data-model)
  - [Buckets](#buckets)
  - [Manifests](#manifests)
  - [Nodes](#nodes)
  - [Pins](#pins)
- [Cryptography](#cryptography)
  - [Identity](#identity)
  - [Key Sharing](#key-sharing)
  - [Content Encryption](#content-encryption)
- [Peer Structure](#peer-structure)
- [Synchronization Protocol](#synchronization-protocol)
- [Security Model](#security-model)

## Overview

JaxBucket is a peer-to-peer, encrypted storage system that combines:

1. **Content Addressing**: Files and directories stored as BLAKE3-hashed blobs
2. **Encryption**: Each file/directory has its own encryption key
3. **P2P Networking**: Built on Iroh's QUIC-based networking stack
4. **Merkle DAGs**: Immutable, hash-linked data structures

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

## Data Model

### Buckets

A **bucket** is a versioned, encrypted collection of files and directories. Each bucket is identified by a **UUID** and contains:

- **Manifest**: Current state of the bucket (unencrypted metadata)
- **Root Node**: Encrypted directory structure
- **Blobs**: Encrypted file contents
- **Version Chain**: Link to previous manifest version

Buckets form a **version history** where each manifest points to its predecessor, creating an immutable audit trail.

### Manifests

The manifest is the entry point to a bucket. It contains unencrypted metadata about the bucket's structure and access control.

**Location**: `rust/crates/common/src/bucket/manifest.rs:54`

```rust
pub struct Manifest {
    pub id: Uuid,                    // Global bucket identifier
    pub name: String,                // Display name (not unique)
    pub shares: Shares,              // Access control list
    pub entry: Link,                 // Points to root Node
    pub pins: Link,                  // Points to Pins (HashSeq)
    pub previous: Option<Link>,      // Previous manifest version
    pub version: Version,            // Software version metadata
}
```

**Key Fields:**

- **`id`**: UUID that uniquely identifies this bucket across all peers
- **`name`**: Human-readable label (can be changed, not guaranteed unique)
- **`shares`**: Map of `PublicKey -> BucketShare` defining who can access the bucket
- **`entry`**: Content-addressed link (CID) pointing to the encrypted root directory node
- **`pins`**: Link to a HashSeq containing all content hashes that should be kept locally
- **`previous`**: Link to the prior manifest version (forms version chain)
- **`version`**: Software version that created this manifest

**Serialization:**
- Manifests are serialized using **DAG-CBOR** (IPLD)
- Stored as raw blobs in Iroh's BlobStore
- Addressed by their BLAKE3 hash

### Nodes

A **node** represents a directory in the bucket's file tree. Nodes are **encrypted** and **content-addressed**.

**Location**: `rust/crates/common/src/bucket/node.rs`

```rust
pub struct Node {
    pub links: BTreeMap<String, NodeLink>,
}

pub enum NodeLink {
    Data(Link, Secret, Metadata),  // File
    Dir(Link, Secret),             // Subdirectory
}

pub struct Metadata {
    pub mime_type: Option<String>,
    pub custom: BTreeMap<String, String>,
}
```

**Structure:**

- **`links`**: Sorted map of name → NodeLink
  - Keys are file/directory names (e.g., `"README.md"`, `"src"`)
  - Values describe the target (file or subdirectory)

**NodeLink Variants:**

1. **`Data(link, secret, metadata)`**: Represents a file
   - `link`: Content-addressed pointer to encrypted file blob
   - `secret`: Encryption key for decrypting the file
   - `metadata`: MIME type and custom properties

2. **`Dir(link, secret)`**: Represents a subdirectory
   - `link`: Content-addressed pointer to child Node
   - `secret`: Encryption key for decrypting the child Node

**Encryption:**

1. Node is serialized to DAG-CBOR
2. Encrypted with ChaCha20-Poly1305 using the node's secret key
3. Stored as a blob
4. Addressed by BLAKE3 hash of the ciphertext

**Example:**

```text
Root Node (encrypted with bucket secret):
{
  "README.md": Data(QmABC..., [secret], {mime: "text/markdown"}),
  "src":       Dir(QmXYZ..., [secret])
}
  └─> src Node (encrypted with its own secret):
      {
        "main.rs": Data(QmDEF..., [secret], {mime: "text/rust"}),
        "lib.rs":  Data(QmGHI..., [secret], {mime: "text/rust"})
      }
```

### Pins

**Pins** define which content should be kept locally. They prevent garbage collection of important blobs.

**Location**: `rust/crates/common/src/bucket/pins.rs`

```rust
pub struct Pins(pub HashSet<Hash>);
```

**Format:**
- Set of BLAKE3 hashes representing blobs to keep
- Serialized as an Iroh **HashSeq** (ordered list of hashes)
- Stored as a blob, linked from the manifest

**Usage:**

When saving a bucket:
1. Collect all Node and file blob hashes
2. Add them to the Pins set
3. Serialize as HashSeq and store
4. Manifest's `pins` field points to this HashSeq

When syncing:
1. Download the pins HashSeq
2. Verify all pinned content is available
3. Download missing blobs from peers

## Cryptography

### Identity

Each peer has an **Ed25519 keypair** as their identity.

**Location**: `rust/crates/common/src/crypto/keys.rs`

```rust
pub struct SecretKey(ed25519_dalek::SigningKey);  // 32 bytes
pub struct PublicKey(ed25519_dalek::VerifyingKey); // 32 bytes
```

**Properties:**
- **SecretKey**: Stored in `~/.config/jax/secret.pem` (PEM format)
- **PublicKey**: Derived from secret key, used as Node ID
- **Dual Purpose**:
  1. Network identity (Iroh uses PublicKey as NodeId)
  2. Encryption key sharing (converted to X25519 for ECDH)

**Key Generation:**
```rust
let secret_key = SecretKey::generate();
let public_key = secret_key.public_key();
```

### Key Sharing

Buckets are shared between peers using **ECDH + AES Key Wrap**.

**Location**: `rust/crates/common/src/crypto/share.rs`

**Protocol:**

To share a bucket secret with another peer:

1. **Generate Ephemeral Key**: Create temporary Ed25519 keypair
2. **ECDH**: Convert both keys to X25519 and compute shared secret
   ```rust
   let shared_secret = ecdh(ephemeral_secret, recipient_public);
   ```
3. **AES Key Wrap**: Wrap the bucket secret using shared secret (RFC 3394)
   ```rust
   let wrapped = aes_kw::wrap(kek: shared_secret, secret: bucket_secret);
   ```
4. **Package Share**: Combine ephemeral public key + wrapped secret
   ```rust
   Share = [ephemeral_pubkey(32 bytes) || wrapped_secret(40 bytes)]
   // Total: 72 bytes
   ```

**Unwrapping:**

The recipient recovers the secret:

1. Extract ephemeral public key from Share (first 32 bytes)
2. Compute ECDH with their private key
   ```rust
   let shared_secret = ecdh(my_secret, ephemeral_public);
   ```
3. Unwrap the secret using AES-KW
   ```rust
   let bucket_secret = aes_kw::unwrap(kek: shared_secret, wrapped);
   ```

**BucketShare Structure:**

```rust
pub struct BucketShare {
    pub principal: Principal,
    pub share: Share,
}

pub struct Principal {
    pub role: PrincipalRole,  // Owner, Editor, Viewer
    pub identity: PublicKey,  // Peer's public key
}

pub enum PrincipalRole {
    Owner,   // Full control
    Editor,  // Read + Write
    Viewer,  // Read only
}
```

### Content Encryption

Files and nodes are encrypted with **ChaCha20-Poly1305 AEAD**.

**Location**: `rust/crates/common/src/crypto/secret.rs`

```rust
pub struct Secret([u8; 32]);  // 256-bit key
```

**Encryption Process:**

1. **Generate Nonce**: Random 96-bit nonce (12 bytes)
2. **Encrypt**: Use ChaCha20-Poly1305
   ```rust
   let cipher = ChaCha20Poly1305::new(&secret);
   let ciphertext = cipher.encrypt(&nonce, plaintext)?;
   ```
3. **Format**: `nonce(12) || ciphertext || tag(16)`
4. **Hash**: Compute BLAKE3 hash of the encrypted blob
5. **Store**: Save blob with hash as address

**Properties:**
- **Per-Item Keys**: Each file and node has its own Secret
- **Content Addressing**: Hashes are stable (computed after encryption)
- **Fine-Grained Access**: Can share individual file keys without exposing entire bucket
- **Authentication**: AEAD provides tamper detection

**Decryption:**

1. Extract nonce (first 12 bytes)
2. Decrypt remaining bytes
   ```rust
   let plaintext = cipher.decrypt(&nonce, ciphertext)?;
   ```
3. Verify AEAD tag (automatic, failure = tampered data)

## Peer Structure

A JaxBucket peer consists of:

### 1. Identity

**Ed25519 keypair** stored in `secret.pem`:
- Private key for decryption and signing
- Public key serves as Node ID

### 2. BlobStore

**Iroh's content-addressed storage**:
- Stores encrypted nodes and files
- Deduplicates by BLAKE3 hash
- Supports Raw blobs and HashSeq collections
- Local cache on disk

**Location**: `~/.config/jax/blobs/`

### 3. Endpoint

**Iroh's QUIC networking**:
- NAT traversal via STUN/TURN
- DHT-based peer discovery (Mainline DHT)
- Multiple ALPN protocols:
  - `iroh-blobs`: For blob transfer
  - `jax-protocol`: For sync messages

### 4. Database

**SQLite database** for metadata:
- Bucket manifests
- Current bucket links
- Sync status
- Peer relationships

**Location**: `~/.config/jax/jax.db`

## Synchronization Protocol

JaxBucket implements a custom P2P sync protocol on top of Iroh.

**Location**: `rust/crates/common/src/peer/jax_protocol/`

### Protocol Messages

**Location**: `rust/crates/common/src/peer/jax_protocol/messages.rs`

```rust
pub enum Request {
    Ping(PingRequest),
    FetchBucket(FetchBucketRequest),
}

pub enum Response {
    Ping(PingResponse),
    FetchBucket(FetchBucketResponse),
}

pub enum Message {
    Announce(AnnounceMessage),
}
```

#### 1. Ping

Check if a peer has updates for a bucket:

```rust
PingRequest {
    bucket_id: Uuid,
    current_link: Option<Link>,  // Our current version
}

PingResponse {
    status: SyncStatus,
}

enum SyncStatus {
    NotFound,   // Peer doesn't have this bucket
    Behind,     // Peer is behind us
    InSync,     // Same version
    Ahead,      // Peer has updates
}
```

#### 2. Fetch

Retrieve a bucket's current link:

```rust
FetchBucketRequest {
    bucket_id: Uuid,
}

FetchBucketResponse {
    current_link: Option<Link>,  // None if peer doesn't have bucket
}
```

#### 3. Announce

Notify peers of a new bucket version (fire-and-forget):

```rust
AnnounceMessage {
    bucket_id: Uuid,
    new_link: Link,
    previous_link: Option<Link>,
}
```

### Sync Workflow

**Location**: `rust/crates/service/src/sync_manager/mod.rs`

#### Pull Sync

Fetch updates from peers:

1. **Query Peers**: Ping all peers for the bucket in parallel
2. **Find Ahead Peer**: Look for a peer with `SyncStatus::Ahead`
3. **Fetch Link**: Get the new bucket link from that peer
4. **Download Manifest**: Fetch the latest manifest blob for that link from the specific peer
5. **Multi-Hop Verify**: Walk the manifest chain backwards until a manifest whose `previous` equals our current link is found, bounded by `MAX_HISTORY_DEPTH`
6. **Verify Provenance**: Check that the announcing peer is authorized in the bucket shares
7. **Download Pins**: Fetch pinset (HashSeq)
8. **Update Database**: Save new link, mark as synced

**Code**: see `verify_multi_hop` and `verify_and_apply_update` in `rust/crates/service/src/sync_manager/mod.rs`

#### Push Sync

Announce updates to peers:

1. **Get Peer List**: Query database for bucket peers
2. **Read Manifest**: Extract previous link
3. **Send Announces**: Fire announce messages to all peers in parallel
4. **Log Results**: Best-effort delivery, no retries

**Code**: `rust/crates/service/src/sync_manager/mod.rs:420`

#### Announce Handler

Process incoming announcements:

1. **Receive Announce**: Peer notifies us of new version
2. **Verify Provenance**: Check peer is in bucket shares
3. **Download Manifest**: Fetch latest manifest referenced by the announced link from the announcing peer
4. **Multi-Hop Verify**: Walk the peer's manifest chain back to our current link within `MAX_HISTORY_DEPTH`
5. **Download Pins**: Fetch pinset
6. **Update Database**: Save new link

**Code**: see `verify_multi_hop` and `verify_and_apply_update` in `rust/crates/service/src/sync_manager/mod.rs`

### Sync Verification

Core verification checks:

#### 1. Provenance Check

Only accept updates from authorized peers:

```rust
if !manifest.shares.contains_key(&peer_public_key) {
    return Err("Unauthorized peer");
}
```

#### 2. Multi-Hop Verification

Accept updates only if the peer's latest link chains back to our current link within a bounded history depth. The verifier walks the `previous` pointers starting from the peer's latest manifest, downloading only from the specific peer, until it finds a manifest whose `previous` equals our current link or the walk terminates.

Algorithm (simplified):

```rust
for depth in 0..MAX_HISTORY_DEPTH {
    let manifest = download_or_use_cached(latest_or_cursor, peer_pub_key)?;
    match manifest.previous() {
        Some(prev) if prev == our_current_link => return Verified(depth),
        Some(prev) => cursor = prev.clone(),
        None => return Fork,
    }
}
return DepthExceeded
```

Outcomes:
- **Verified(depth)**: Update is on the same chain; safe to apply
- **Fork**: Peer chain does not include our current link; reject update
- **DepthExceeded**: Chain too long (over `MAX_HISTORY_DEPTH`); reject update

Depth is bounded by `MAX_HISTORY_DEPTH` (see `rust/crates/service/src/jax_state.rs`, default 100) to protect against unbounded history walks.

On failure (Fork or DepthExceeded), the update is rejected and the sync status is marked as Failed. A full pull sync may be initiated separately to reconcile state if desired.

## Security Model

### Threat Model

**JaxBucket protects against:**

✅ **Untrusted Storage Providers**
- All blobs are encrypted
- Storage provider sees only hashes
- Cannot decrypt content without keys

✅ **Passive Network Observers**
- QUIC provides TLS 1.3 encryption
- Peer connections are authenticated
- Traffic is encrypted end-to-end

✅ **Unauthorized Peers**
- Only peers with valid BucketShare can decrypt
- ECDH ensures only recipient can unwrap secrets
- Access control enforced via shares list

✅ **Tampered Data**
- AEAD detects modifications
- Content addressing ensures integrity
- Hash verification on all blobs

**JaxBucket does NOT protect against:**

❌ **Compromised Peer with Valid Access**
- If an authorized peer is compromised, attacker gains access
- No forward secrecy or key rotation (yet)
- Recommendation: Regularly audit shares list

❌ **Malicious Authorized Peer**
- Authorized peers can leak data
- Trust model assumes peers with access are trustworthy
- Recommendation: Only share with trusted devices/users

❌ **Metadata Leakage**
- Bucket structure visible (file count, sizes, hierarchy)
- Storage provider can see blob access patterns
- Recommendation: Use padding or cover traffic (future work)

❌ **Traffic Analysis**
- Connection patterns may reveal peer relationships
- Sync frequency might leak activity patterns
- Recommendation: Use Tor or mixnets (future work)

❌ **Side-Channel Attacks**
- Timing attacks on crypto operations
- Power analysis (if physical access)
- Recommendation: Use constant-time crypto (mostly implemented)

### Best Practices

1. **Protect Secret Keys**
   - Store `secret.pem` with `chmod 600`
   - Back up securely (encrypted, offline)
   - Never share or commit to version control

2. **Verify Peer Identity**
   - Check public key fingerprints out-of-band
   - Use QR codes or secure channels for initial sharing

3. **Regular Key Rotation**
   - Periodically rotate bucket secrets (manual process currently)
   - Remove old shares when no longer needed

4. **Audit Access**
   - Regularly review bucket shares
   - Remove peers that no longer need access

5. **Monitor Sync Activity**
   - Watch for unexpected updates
   - Investigate unknown peers or sync patterns

### Future Security Enhancements

- [ ] Forward secrecy via key rotation
- [ ] Access revocation with re-encryption
- [ ] Metadata padding to hide structure
- [ ] Traffic obfuscation
- [ ] Formal security audit

## Implementation Details

### Key Files

- **Manifest**: `rust/crates/common/src/bucket/manifest.rs`
- **Node**: `rust/crates/common/src/bucket/node.rs`
- **Pins**: `rust/crates/common/src/bucket/pins.rs`
- **Keys**: `rust/crates/common/src/crypto/keys.rs`
- **Secret**: `rust/crates/common/src/crypto/secret.rs`
- **Share**: `rust/crates/common/src/crypto/share.rs`
- **Link**: `rust/crates/common/src/linked_data/link.rs`
- **Peer**: `rust/crates/common/src/peer/mod.rs`
- **JAX Protocol**: `rust/crates/common/src/peer/jax_protocol/`
- **Sync Manager**: `rust/crates/service/src/sync_manager/mod.rs`
- **Service Constants**: `rust/crates/service/src/jax_state.rs` (e.g., `MAX_HISTORY_DEPTH`)

### Dependencies

- **Iroh**: P2P networking and blob storage
- **ed25519-dalek**: Identity keypairs
- **chacha20poly1305**: Content encryption
- **aes-kw**: Key wrapping (RFC 3394)
- **blake3**: Content addressing (via Iroh)
- **serde_ipld_dagcbor**: DAG-CBOR serialization

## References

- **Iroh**: https://iroh.computer/
- **IPLD**: https://ipld.io/
- **RFC 3394** (AES Key Wrap): https://tools.ietf.org/html/rfc3394
- **ChaCha20-Poly1305**: https://tools.ietf.org/html/rfc8439
- **Ed25519**: https://ed25519.cr.yp.to/
- **BLAKE3**: https://github.com/BLAKE3-team/BLAKE3
