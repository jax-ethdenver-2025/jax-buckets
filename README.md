# JaxBucket

[![Crates.io](https://img.shields.io/crates/v/jax-common.svg)](https://crates.io/crates/jax-common)
[![Documentation](https://docs.rs/jax-bucket/badge.svg)](https://docs.rs/jax-bucket)
[![CI](https://github.com/jax-ethdenver-2025/jax-buckets/actions/workflows/ci-rust.yml/badge.svg)](https://github.com/jax-ethdenver-2025/jax-buckets/actions/workflows/ci-rust.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.75%2B-blue.svg)](https://www.rust-lang.org)

**End-to-End Encrypted Storage Buckets with Peer-to-Peer Synchronization**

> **⚠️ SECURITY DISCLAIMER**
>
> **This software has NOT been audited by security professionals and is NOT production-ready.**
>
> JaxBucket is an experimental project built for learning and demonstration purposes. The cryptographic implementation and protocol design have not undergone formal security review. Do not use this software to protect sensitive, confidential, or production data.
>
> Use at your own risk. The authors assume no liability for data loss, security breaches, or other issues arising from the use of this software.

## Overview

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

## Quick Start

```bash
# Install JaxBucket
cargo install jax-bucket

# Initialize configuration
jax init

# Start the service
jax service

# Open web UI at http://localhost:8080
```

For detailed installation instructions and requirements, see [INSTALL.md](INSTALL.md).

## Documentation

- **[INSTALL.md](INSTALL.md)** - Installation instructions and system requirements
- **[USAGE.md](USAGE.md)** - How to use JaxBucket (CLI, Web UI, API)
- **[PROTOCOL.md](PROTOCOL.md)** - Technical protocol specification and data model
- **[DEVELOPMENT.md](DEVELOPMENT.md)** - Development setup and environment
- **[CONTRIBUTING.md](CONTRIBUTING.md)** - Contribution guidelines
- **[rust/RELEASE.md](rust/RELEASE.md)** - Release process

## Use Cases

- **Personal Cloud**: Sync files between your devices without trusting a cloud provider
- **Collaborative Workspaces**: Share encrypted folders with team members
- **Backup & Archive**: Distributed, encrypted backups across multiple machines
- **Research**: Experiment with content-addressed, encrypted storage systems

## Project Structure

```text
jax-bucket/
├── rust/
│   └── crates/
│       ├── common/          # Core data structures and crypto
│       ├── service/         # HTTP server and sync manager
│       └── app/             # CLI binary
├── README.md                # This file
├── INSTALL.md               # Installation guide
├── USAGE.md                 # Usage guide
├── PROTOCOL.md              # Protocol specification
├── DEVELOPMENT.md           # Development guide
└── CONTRIBUTING.md          # Contribution guidelines
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT License - see [LICENSE](LICENSE) for details

## Acknowledgments

Built with:
- **[Iroh](https://iroh.computer/)** - P2P networking and content storage
- **[Rust](https://www.rust-lang.org/)** - Systems programming language
- **[DAG-CBOR](https://ipld.io/)** - Merkle DAG serialization

## Contact

- **Issues**: https://github.com/jax-ethdenver-2025/jax-bucket/issues
- **Discussions**: https://github.com/jax-ethdenver-2025/jax-bucket/discussions
