//! Bucket data structures and operations
//!
//! This module defines the core types for JaxBucket's encrypted, content-addressed file storage:
//!
//! - **[`Manifest`]**: Bucket metadata including ID, name, shares, and content-addressed pointers
//! - **[`Node`]**: DAG structure representing directories and files
//! - **[`Mount`]**: In-memory representation of a bucket with CRUD operations
//! - **[`Pins`]**: Set of content hashes that should be kept available
//! - **[`Principal`]**: Access control entries (peer identity + role)
//!
//! # Architecture
//!
//! ## Buckets as DAGs
//!
//! A bucket is a Directed Acyclic Graph (DAG) of encrypted nodes:
//! ```text
//! Manifest (unencrypted) --entry--> Root Node (encrypted)
//!                                       |
//!                    +------------------+------------------+
//!                    |                  |                  |
//!                  File1            Dir Node             File2
//!                (encrypted)       (encrypted)        (encrypted)
//!                                      |
//!                              +-------+-------+
//!                              |               |
//!                            File3           File4
//!                         (encrypted)     (encrypted)
//! ```
//!
//! ## Content Addressing
//!
//! All nodes and files are content-addressed by their (post-encryption) hash.
//! Links between nodes use [`Link`](crate::linked_data::Link), which includes:
//! - Hash (BLAKE3)
//! - Codec (DAG-CBOR for nodes, Raw for encrypted data)
//! - Format (Raw blob or HashSeq)
//!
//! ## Encryption Model
//!
//! - Each node and file has its own encryption [`Secret`](crate::crypto::Secret)
//! - Secrets are stored in the parent node's [`NodeLink`]
//! - The root node's secret is shared with authorized peers via [`Share`](crate::crypto::Share)
//! - This provides fine-grained access control and efficient key rotation

mod manifest;
mod maybe_mime;
mod mount;
mod node;
mod pins;
mod principal;

pub use manifest::Manifest;
pub use mount::{Mount, MountError};
pub use node::{Node, NodeError, NodeLink};
pub use pins::Pins;
