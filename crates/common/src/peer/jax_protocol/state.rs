use async_trait::async_trait;
use uuid::Uuid;

use crate::crypto::SecretKey;
use crate::linked_data::Link;
use crate::peer::BlobsStore;

use super::messages::SyncStatus;

/// Information about a peer that has access to a bucket
#[derive(Debug, Clone)]
pub struct ShareInfo {
    pub public_key: String,
    pub role: String,
}

/// Sync status for a bucket
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BucketSyncStatus {
    Synced,
    OutOfSync,
    Syncing,
    Failed,
}

/// Trait for providing state to the Peer and protocol handlers
///
/// This trait abstracts away the storage layer (database + blobs) so that
/// the peer and protocol handlers in `common` can access state without depending
/// on the `service` crate.
#[async_trait]
pub trait PeerStateProvider: Send + Sync + std::fmt::Debug {
    // ===== Bucket Queries =====

    /// Check the sync status of a bucket given a target link
    ///
    /// This compares the target_link against the current state of the bucket:
    /// - NotFound: The bucket doesn't exist
    /// - InSync: The target_link matches the current bucket link
    /// - Ahead: We are ahead of the target_link (target is in our history)
    /// - Behind: We are behind the target_link (we need to sync)
    async fn check_bucket_sync(
        &self,
        bucket_id: Uuid,
        target_link: &Link,
    ) -> Result<SyncStatus, anyhow::Error>;

    /// Get the current link for a bucket
    ///
    /// Returns None if the bucket doesn't exist
    async fn get_bucket_link(&self, bucket_id: Uuid) -> Result<Option<Link>, anyhow::Error>;

    /// Get all shares (peers) for a bucket
    async fn get_bucket_shares(&self, bucket_id: Uuid) -> Result<Vec<ShareInfo>, anyhow::Error>;

    // ===== Bucket Mutations =====

    /// Update the bucket link in storage
    async fn update_bucket_link(
        &self,
        bucket_id: Uuid,
        new_link: Link,
    ) -> Result<(), anyhow::Error>;

    /// Update the bucket link and mark as synced (combined operation)
    async fn update_bucket_link_and_sync(
        &self,
        bucket_id: Uuid,
        new_link: Link,
    ) -> Result<(), anyhow::Error>;

    /// Update sync status for a bucket
    async fn update_sync_status(
        &self,
        bucket_id: Uuid,
        status: BucketSyncStatus,
        error: Option<String>,
    ) -> Result<(), anyhow::Error>;

    /// Create a new bucket from a peer (for first-time sync)
    async fn create_bucket(
        &self,
        bucket_id: Uuid,
        name: String,
        link: Link,
    ) -> Result<(), anyhow::Error>;

    // ===== Data Access =====

    /// Access to the blobs store
    fn blobs(&self) -> &BlobsStore;

    /// Access to the node's secret key
    fn node_secret(&self) -> &SecretKey;
}
