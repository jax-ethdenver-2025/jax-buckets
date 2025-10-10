//! JAX Protocol - Custom ALPN protocol for peer status checking and bucket sync
//!
//! This module implements a custom iroh protocol for checking:
//! - Whether a peer is online
//! - Whether a peer has a specific bucket
//! - The sync status of a bucket between peers

mod client;
mod handler;
mod messages;
mod state;

pub use client::ping_peer;
pub use handler::{JaxProtocol, JAX_ALPN};
pub use messages::{PingRequest, PingResponse, SyncStatus};
pub use state::BucketStateProvider;
