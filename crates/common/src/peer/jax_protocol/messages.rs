use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::linked_data::Link;

/// Top-level request enum for the JAX protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    /// Ping request to check sync status
    Ping(PingRequest),
    /// Fetch bucket request to get current link
    FetchBucket(FetchBucketRequest),
    /// Announce message (one-way, no response expected)
    Announce(AnnounceMessage),
}

/// Top-level response enum for the JAX protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    /// Ping response with sync status
    Ping(PingResponse),
    /// Fetch bucket response with current link
    FetchBucket(FetchBucketResponse),
}

/// Request to ping a peer and check bucket sync status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingRequest {
    /// The bucket ID to check
    pub bucket_id: Uuid,
    /// The current link the requesting peer has for this bucket
    pub current_link: Link,
}

/// Sync status between two peers for a bucket
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SyncStatus {
    /// The peer does not have this bucket
    NotFound,
    /// The requesting peer's link is in the responding peer's history (requesting peer is behind)
    Behind,
    /// Both peers have the same current link (in sync)
    InSync,
    /// The requesting peer's link is beyond the responding peer's history (responding peer is behind)
    Ahead,
}

/// Response to a ping request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResponse {
    pub status: SyncStatus,
}

impl PingResponse {
    pub fn new(status: SyncStatus) -> Self {
        Self { status }
    }

    pub fn not_found() -> Self {
        Self {
            status: SyncStatus::NotFound,
        }
    }

    pub fn behind() -> Self {
        Self {
            status: SyncStatus::Behind,
        }
    }

    pub fn in_sync() -> Self {
        Self {
            status: SyncStatus::InSync,
        }
    }

    pub fn unsynced() -> Self {
        Self {
            status: SyncStatus::Ahead,
        }
    }
}

/// Announcement of a new bucket version to peers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnounceMessage {
    /// The bucket ID being announced
    pub bucket_id: Uuid,
    /// The new link for this bucket
    pub new_link: Link,
    /// The previous link (for single-hop verification)
    pub previous_link: Option<Link>,
}

impl AnnounceMessage {
    pub fn new(bucket_id: Uuid, new_link: Link, previous_link: Option<Link>) -> Self {
        Self {
            bucket_id,
            new_link,
            previous_link,
        }
    }
}

/// Request to fetch the current bucket link from a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchBucketRequest {
    /// The bucket ID to fetch
    pub bucket_id: Uuid,
}

impl FetchBucketRequest {
    pub fn new(bucket_id: Uuid) -> Self {
        Self { bucket_id }
    }
}

/// Response to a fetch bucket request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchBucketResponse {
    /// The current link for this bucket (None if bucket not found)
    pub current_link: Option<Link>,
}

impl FetchBucketResponse {
    pub fn new(current_link: Option<Link>) -> Self {
        Self { current_link }
    }

    pub fn not_found() -> Self {
        Self { current_link: None }
    }

    pub fn found(link: Link) -> Self {
        Self {
            current_link: Some(link),
        }
    }
}
