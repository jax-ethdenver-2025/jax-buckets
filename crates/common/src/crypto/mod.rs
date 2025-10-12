//! Cryptographic primitives for JaxBucket
//!
//! This module provides the cryptographic foundation for JaxBucket's security model:
//!
//! - **Identity & Authentication**: Ed25519 keypairs for peer identity
//! - **Encryption**: AES-256-GCM for content encryption with per-item secrets
//! - **Key Sharing**: ECDH-based key sharing using X25519 curve conversion
//!
//! # Security Model
//!
//! ## Peer Identity
//! Each peer has an Ed25519 keypair (`SecretKey`/`PublicKey`) that serves as their
//! identity in the network. This same keypair is used for key sharing.
//!
//! ## Content Encryption
//! Every encrypted item (nodes, data) has its own AES-256-GCM `Secret` key. This provides:
//! - Content-addressed storage (hashes are stable)
//! - Per-item encryption (no shared secrets across items)
//! - Forward secrecy (rotating keys doesn't require re-encryption)
//!
//! ## Key Sharing Protocol
//! To share a bucket with another peer:
//! 1. Generate ephemeral Ed25519 keypair
//! 2. Convert both peer's Ed25519 keys to X25519 (Montgomery curve)
//! 3. Perform ECDH to derive shared secret
//! 4. Use AES-KW (key wrap) to encrypt the bucket secret with shared secret
//! 5. Package as a `Share` (ephemeral_pubkey || wrapped_secret)
//!
//! The recipient can recover the secret by:
//! 1. Extracting the ephemeral public key from the Share
//! 2. Converting keys to X25519
//! 3. Performing ECDH with their private key
//! 4. Using AES-KW to unwrap the secret

mod keys;
mod secret;
mod share;

pub use keys::{PublicKey, SecretKey};
pub use secret::{Secret, SecretError};
pub use share::{Share, ShareError};
