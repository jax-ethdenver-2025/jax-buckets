use async_trait::async_trait;
use std::collections::HashSet;
use uuid::Uuid;

use common::bucket::Manifest;
use common::crypto::SecretKey;
use common::linked_data::{BlockEncoded, Link};
use common::peer::{BlobsStore, BucketSyncStatus, PeerStateProvider, ShareInfo, SyncStatus};

use crate::database::models::SyncStatus as DbSyncStatus;
use crate::database::{models::Bucket, Database};

/// Maximum depth to traverse when checking bucket history
pub const MAX_HISTORY_DEPTH: usize = 100;

/// State implementation for the peer
///
/// This provides read-only and write access to bucket state
/// for the peer and protocol handlers.
#[derive(Clone)]
pub struct ServicePeerState {
    database: Database,
    blobs: BlobsStore,
    node_secret: SecretKey,
}

impl std::fmt::Debug for ServicePeerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServicePeerState")
            .field("database", &self.database)
            .field("blobs", &"<BlobsStore>")
            .field("node_secret", &"<SecretKey>")
            .finish()
    }
}

impl ServicePeerState {
    pub fn new(database: Database, blobs: BlobsStore, node_secret: SecretKey) -> Self {
        Self {
            database,
            blobs,
            node_secret,
        }
    }

    pub fn database(&self) -> &Database {
        &self.database
    }

    /// Load a BucketData from a link
    async fn load_bucket_data(&self, link: &Link) -> Result<Manifest, anyhow::Error> {
        let data = self.blobs.get(link.hash()).await?;
        Ok(Manifest::decode(&data)?)
    }

    /// Check if a target link is in the bucket's history
    ///
    /// Returns:
    /// - Some(true) if the link is found (target is an ancestor)
    /// - Some(false) if we reached max depth without finding it
    /// - None if we exhausted the history without finding it
    async fn is_link_in_history(
        &self,
        current_link: &Link,
        target_link: &Link,
    ) -> Result<Option<bool>, anyhow::Error> {
        let mut seen_links = HashSet::new();
        let mut current = current_link.clone();
        let mut depth = 0;

        tracing::debug!(
            "Checking if link {:?} is in history of {:?}",
            target_link,
            current_link
        );

        seen_links.insert(current.clone());

        while depth < MAX_HISTORY_DEPTH {
            // Load the bucket data
            let bucket_data = match self.load_bucket_data(&current).await {
                Ok(data) => data,
                Err(e) => {
                    tracing::warn!("Failed to load bucket data at link {:?}: {}", current, e);
                    return Ok(Some(false));
                }
            };

            // Check if there's a previous version
            let Some(previous_link) = bucket_data.previous().clone() else {
                tracing::debug!("No more history after {:?}", current);
                return Ok(None);
            };

            // Check if we've found the target
            if &previous_link == target_link {
                tracing::debug!("Found target link in history, we are ahead");
                return Ok(Some(true));
            }

            // Avoid cycles
            if seen_links.contains(&previous_link) {
                tracing::warn!("Cycle detected in bucket history");
                return Ok(Some(false));
            }

            seen_links.insert(previous_link.clone());
            current = previous_link;
            depth += 1;
        }

        // Hit max depth
        Ok(Some(false))
    }
}

#[async_trait]
impl PeerStateProvider for ServicePeerState {
    async fn check_bucket_sync(
        &self,
        bucket_id: Uuid,
        target_link: &Link,
    ) -> Result<SyncStatus, anyhow::Error> {
        // Get the bucket from the database
        let bucket = match Bucket::get_by_id(&bucket_id, &self.database).await? {
            Some(b) => b,
            None => return Ok(SyncStatus::NotFound),
        };

        let current_link: Link = bucket.link.into();

        // If the links match, we're in sync
        if &current_link == target_link {
            return Ok(SyncStatus::InSync);
        }

        // Check if the target is in our history (target is behind)
        match self.is_link_in_history(&current_link, target_link).await? {
            // We are ahead
            Some(true) => Ok(SyncStatus::Ahead),
            _ => {
                // Either not found or hit max depth
                // In this case, we're behind
                Ok(SyncStatus::Behind)
            }
        }
    }

    async fn get_bucket_link(&self, bucket_id: Uuid) -> Result<Option<Link>, anyhow::Error> {
        // Get the bucket from the database
        let bucket = Bucket::get_by_id(&bucket_id, &self.database).await?;
        Ok(bucket.map(|b| b.link.into()))
    }

    async fn get_bucket_shares(&self, bucket_id: Uuid) -> Result<Vec<ShareInfo>, anyhow::Error> {
        // Get the bucket from database
        let bucket = Bucket::get_by_id(&bucket_id, &self.database)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Bucket {} not found", bucket_id))?;

        let bucket_link: Link = bucket.link.into();

        // Load the manifest to get shares
        let manifest = self.load_bucket_data(&bucket_link).await?;

        // Convert shares to ShareInfo
        let shares: Vec<ShareInfo> = manifest
            .shares()
            .values()
            .map(|share| ShareInfo {
                public_key: share.principal().identity.to_hex(),
                role: format!("{:?}", share.principal().role),
            })
            .collect();

        Ok(shares)
    }

    async fn update_bucket_link(
        &self,
        bucket_id: Uuid,
        new_link: Link,
    ) -> Result<(), anyhow::Error> {
        let bucket = Bucket::get_by_id(&bucket_id, &self.database)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Bucket {} not found", bucket_id))?;

        bucket
            .update_link(new_link, &self.database)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update bucket link: {}", e))
    }

    async fn update_bucket_link_and_sync(
        &self,
        bucket_id: Uuid,
        new_link: Link,
    ) -> Result<(), anyhow::Error> {
        let bucket = Bucket::get_by_id(&bucket_id, &self.database)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Bucket {} not found", bucket_id))?;

        bucket
            .update_link_and_sync(new_link, &self.database)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update bucket link: {}", e))
    }

    async fn update_sync_status(
        &self,
        bucket_id: Uuid,
        status: BucketSyncStatus,
        error: Option<String>,
    ) -> Result<(), anyhow::Error> {
        let bucket = Bucket::get_by_id(&bucket_id, &self.database)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Bucket {} not found", bucket_id))?;

        // Convert common BucketSyncStatus to database SyncStatus
        let db_status = match status {
            BucketSyncStatus::Synced => DbSyncStatus::Synced,
            BucketSyncStatus::OutOfSync => DbSyncStatus::OutOfSync,
            BucketSyncStatus::Syncing => DbSyncStatus::Syncing,
            BucketSyncStatus::Failed => DbSyncStatus::Failed,
        };

        bucket
            .update_sync_status(db_status, error, &self.database)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update sync status: {}", e))
    }

    async fn create_bucket(
        &self,
        bucket_id: Uuid,
        name: String,
        link: Link,
    ) -> Result<(), anyhow::Error> {
        Bucket::create(bucket_id, name, link, &self.database)
            .await
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("Failed to create bucket: {}", e))
    }

    fn blobs(&self) -> &BlobsStore {
        &self.blobs
    }

    fn node_secret(&self) -> &SecretKey {
        &self.node_secret
    }
}
