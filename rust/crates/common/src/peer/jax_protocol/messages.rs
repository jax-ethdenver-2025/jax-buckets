use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::linked_data::Link;

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
    Unsynced,
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
            status: SyncStatus::Unsynced,
        }
    }
}
