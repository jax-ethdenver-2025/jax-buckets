use async_trait::async_trait;
use uuid::Uuid;

use crate::linked_data::Link;

use super::messages::SyncStatus;

/// Trait for providing bucket state information to the JAX protocol handler
///
/// This trait abstracts away the storage layer (database + blobs) so that
/// the protocol handler in `common` can query bucket state without depending
/// on the `service` crate.
#[async_trait]
pub trait BucketStateProvider: Send + Sync + std::fmt::Debug {
    /// Check the sync status of a bucket given a target link
    ///
    /// This compares the target_link against the current state of the bucket:
    /// - NotFound: The bucket doesn't exist
    /// - InSync: The target_link matches the current bucket link
    /// - Behind: The target_link is in the bucket's history (older version)
    /// - Unsynced: The target_link is not in the bucket's history (different branch or newer)
    async fn check_bucket_sync(
        &self,
        bucket_id: Uuid,
        target_link: &Link,
    ) -> Result<SyncStatus, anyhow::Error>;
}
