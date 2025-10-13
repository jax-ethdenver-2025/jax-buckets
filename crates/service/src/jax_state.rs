use async_trait::async_trait;
use flume::Sender;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};
use uuid::Uuid;

use common::bucket::Manifest;
use common::linked_data::{BlockEncoded, Link};
use common::peer::{BlobsStore, BucketStateProvider, SyncStatus};

use crate::database::models::Bucket;
use crate::database::Database;
use crate::sync_manager::SyncEvent;

/// Maximum depth to traverse when checking bucket history
pub const MAX_HISTORY_DEPTH: usize = 100;

/// State provider for the JAX protocol
///
/// This implements the BucketStateProvider trait and provides access
/// to bucket state via the database and blobs store.
#[derive(Clone)]
pub struct JaxState {
    database: Database,
    blobs: Arc<OnceLock<BlobsStore>>,
    sync_sender: Arc<OnceLock<Sender<SyncEvent>>>,
}

impl std::fmt::Debug for JaxState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JaxState")
            .field("database", &self.database)
            .field("blobs", &"<OnceLock>")
            .field("sync_sender", &"<OnceLock>")
            .finish()
    }
}

impl JaxState {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            blobs: Arc::new(OnceLock::new()),
            sync_sender: Arc::new(OnceLock::new()),
        }
    }

    pub fn set_blobs(&self, blobs: BlobsStore) {
        let _ = self.blobs.set(blobs);
    }

    pub fn set_sync_sender(&self, sender: Sender<SyncEvent>) {
        let _ = self.sync_sender.set(sender);
    }

    fn blobs(&self) -> &BlobsStore {
        self.blobs.get().expect("BlobsStore must be set before use")
    }

    fn sync_sender(&self) -> &Sender<SyncEvent> {
        self.sync_sender
            .get()
            .expect("SyncSender must be set before use")
    }

    /// Load a BucketData from a link
    async fn load_bucket_data(&self, link: &Link) -> Result<Manifest, anyhow::Error> {
        let data = self.blobs().get(link.hash()).await?;
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

        tracing::info!(
            "Checking if link {:?} is in history of {:?}",
            target_link,
            current_link
        );

        seen_links.insert(current.clone());

        while depth < MAX_HISTORY_DEPTH {
            tracing::info!(
                "Checking if link {:?} is in history of {:?} -- depth: {}",
                target_link,
                current_link,
                depth
            );
            // Load the bucket data
            let bucket_data = match self.load_bucket_data(&current).await {
                Ok(data) => data,
                Err(e) => {
                    tracing::warn!("Failed to load bucket data at link {:?}: {}", current, e);
                    return Ok(Some(false));
                }
            };
            tracing::info!("Loaded bucket data @ {:?}", current);

            // Check if there's a previous version
            let Some(previous_link) = bucket_data.previous().clone() else {
                tracing::info!("No more history after {:?}", current);
                // No more history
                return Ok(None);
            };

            tracing::info!("Previous link: {:?}", previous_link);

            // Check if we've found the target
            if &previous_link == target_link {
                tracing::info!("Found target link in history, we are ahead");
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
impl BucketStateProvider for JaxState {
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
            // we are ahead
            Some(true) => Ok(SyncStatus::Ahead),
            _ => {
                // Either not found or hit max depth
                // In this case, we're unsynced
                Ok(SyncStatus::Behind)
            }
        }
    }

    async fn get_bucket_link(&self, bucket_id: Uuid) -> Result<Option<Link>, anyhow::Error> {
        // Get the bucket from the database
        let bucket = Bucket::get_by_id(&bucket_id, &self.database).await?;

        Ok(bucket.map(|b| b.link.into()))
    }

    async fn handle_announce(
        &self,
        bucket_id: Uuid,
        peer_id: String,
        new_link: Link,
        previous_link: Option<Link>,
    ) -> Result<(), anyhow::Error> {
        // Send a PeerAnnounce event to the sync manager
        let event = SyncEvent::PeerAnnounce {
            bucket_id,
            peer_id,
            new_link,
            previous_link,
        };

        self.sync_sender()
            .send(event)
            .map_err(|e| anyhow::anyhow!("Failed to send sync event: {}", e))?;

        Ok(())
    }
}
