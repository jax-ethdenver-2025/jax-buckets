/**
 * Common types that dsecribe core JaxBucket responsibilities.
 *  - Buckets
 *  - Nodes
 */
pub mod bucket;
/**
 * Cryptographic types and operations.
 *  - Public and Private key implementations
 *  - Key-to-key key sharing
 */
pub mod crypto;
/**
 * Internal wrapper around IPLD, renamed to
 *  something a little more down-to-earth.
 * Handles translation to/from IPLD and IrohBlobs
 *  for linked data.
 */
pub mod linked_data;
/**
 * Mount implementation over a bucket.
 */
pub mod mount;
/**
 * Storage layer implementation.
 *  Just a light wrapper around the Iroh-Blobs
 *  protocol and ALPN handler
 */
pub mod peer;
/**
 * Helper for setting build version information
 *  at compile time.
 */
pub mod version;

pub mod prelude {
    pub use crate::bucket::BucketData;
    pub use crate::crypto::{PublicKey, SecretKey};
    pub use crate::linked_data::{multibase, Cid, CidError, Link};
    pub use crate::mount::{Bucket, BucketError};
    // Backward compatibility aliases
    pub use crate::mount::{Mount, MountError};
    pub use crate::peer::Peer;
    pub use crate::version::build_info;
}
