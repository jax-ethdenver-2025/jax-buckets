//! JAX Protocol - Custom ALPN protocol for peer status checking and bucket sync
//!
//! This module implements a custom iroh protocol for:
//! - Checking whether a peer is online
//! - Checking whether a peer has a specific bucket
//! - Checking the sync status of a bucket between peers
//! - Fetching the current bucket link from a peer
//! - Announcing new bucket versions to peers

mod client;
mod handler;
mod messages;
mod state;

pub use client::{announce_to_peer, fetch_bucket, ping_peer};
pub use handler::{JaxProtocol, JAX_ALPN};
pub use messages::{
    AnnounceMessage, FetchBucketRequest, FetchBucketResponse, PingRequest, PingResponse, SyncStatus,
};
pub use state::BucketStateProvider;
