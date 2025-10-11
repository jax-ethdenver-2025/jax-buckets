//! Sync manager for bucket synchronization
//!
//! This module handles all bucket synchronization logic, working closely
//! with the JAX protocol to keep buckets in sync across peers.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::future::join_all;
use std::sync::Arc;
use uuid::Uuid;

use crate::bucket::Manifest;
use crate::crypto::PublicKey;
use crate::linked_data::Link;
use crate::peer::jax_protocol::{
    announce_to_peer, fetch_bucket, ping_peer, JaxCallback, SyncStatus,
};
use crate::peer::NodeAddr;
use iroh::Endpoint;

use super::state::{BucketSyncStatus, SyncStateProvider};

/// Sync manager coordinates bucket synchronization with peers
///
/// The sync manager is tightly coupled with the peer and JAX protocol.
/// It handles pull, push, and peer announce operations.
#[derive(Clone)]
pub struct SyncManager<S>
where
    S: SyncStateProvider,
{
    state: Arc<S>,
    endpoint: Endpoint,
    our_node_id: String, // hex-encoded for comparison
}

impl<S> SyncManager<S>
where
    S: SyncStateProvider + 'static,
{
    /// Create a new sync manager
    pub fn new(state: Arc<S>, endpoint: Endpoint, our_node_id: String) -> Self {
        Self {
            state,
            endpoint,
            our_node_id,
        }
    }

    // ===== Public API =====

    /// Pull the latest version of a bucket from peers
    ///
    /// This checks all peers for the bucket and downloads the latest version
    /// if any peer is ahead of us.
    pub async fn pull(&self, bucket_id: Uuid) -> Result<()> {
        tracing::info!("Pull sync for bucket {}", bucket_id);

        // 1. Get bucket from state
        let bucket = match self.state.get_bucket(bucket_id).await? {
            Some(b) => b,
            None => {
                tracing::warn!("Bucket {} not found for pull sync", bucket_id);
                return Ok(());
            }
        };

        // Update sync status to Syncing
        self.state
            .update_sync_status(bucket_id, BucketSyncStatus::Syncing, None)
            .await?;

        // 2. Get list of peers for this bucket
        let peers = self.get_peers_for_bucket(bucket_id).await?;
        if peers.is_empty() {
            tracing::info!("No peers found for bucket {}", bucket_id);
            self.state
                .update_sync_status(bucket_id, BucketSyncStatus::Synced, None)
                .await?;
            return Ok(());
        }

        let current_link = bucket.link.clone();
        tracing::info!(
            "Pull sync: checking {} peers for bucket {}",
            peers.len(),
            bucket_id
        );

        // 3. Ping all peers in parallel to check sync status
        let ping_futures: Vec<_> = peers
            .iter()
            .map(|peer_addr| {
                let peer = peer_addr.clone();
                let link = current_link.clone();
                let endpoint = self.endpoint.clone();
                async move {
                    match ping_peer(&endpoint, &peer, bucket_id, link).await {
                        Ok(status) => Some((peer, status)),
                        Err(e) => {
                            tracing::warn!("Failed to ping peer {:?}: {}", peer, e);
                            None
                        }
                    }
                }
            })
            .collect();

        let results = join_all(ping_futures).await;

        // 4. Find a peer that's ahead of us
        let ahead_peer = results
            .into_iter()
            .flatten()
            .find(|(_, status)| *status == SyncStatus::Ahead);

        let (peer_addr, _) = match ahead_peer {
            Some(p) => p,
            None => {
                tracing::info!("No peers ahead of us for bucket {}", bucket_id);
                self.state
                    .update_sync_status(bucket_id, BucketSyncStatus::Synced, None)
                    .await?;
                return Ok(());
            }
        };

        tracing::info!("Found ahead peer {:?} for bucket {}", peer_addr, bucket_id);

        // 5. Fetch the current bucket link from the ahead peer
        let new_link = match fetch_bucket(&self.endpoint, &peer_addr, bucket_id).await {
            Ok(Some(link)) => link,
            Ok(None) => {
                tracing::warn!(
                    "Ahead peer {:?} returned no link for bucket {}",
                    peer_addr,
                    bucket_id
                );
                self.state
                    .update_sync_status(
                        bucket_id,
                        BucketSyncStatus::OutOfSync,
                        Some("Peer reported as ahead but has no bucket link".to_string()),
                    )
                    .await?;
                return Ok(());
            }
            Err(e) => {
                tracing::error!(
                    "Failed to fetch bucket link from peer {:?}: {}",
                    peer_addr,
                    e
                );
                self.state
                    .update_sync_status(
                        bucket_id,
                        BucketSyncStatus::Failed,
                        Some(format!("Failed to fetch bucket link: {}", e)),
                    )
                    .await?;
                return Err(e);
            }
        };

        tracing::info!(
            "Fetched new link {:?} from ahead peer {:?} for bucket {}",
            new_link,
            peer_addr,
            bucket_id
        );

        // 6. Download the BucketData from the peer
        let peer_pub_key = PublicKey::from(peer_addr.node_id);
        let bucket_data = match self
            .state
            .download_bucket_from_peer(&new_link, &peer_pub_key)
            .await
        {
            Ok(data) => data,
            Err(e) => {
                tracing::error!(
                    "Failed to download bucket data from peer {:?} for link {:?}: {}",
                    peer_addr,
                    new_link,
                    e
                );
                self.state
                    .update_sync_status(
                        bucket_id,
                        BucketSyncStatus::Failed,
                        Some(format!("Failed to download new bucket data: {}", e)),
                    )
                    .await?;
                return Err(e);
            }
        };

        // 7. Verify single-hop: peer's previous must equal our current link
        if !verify_single_hop(&current_link, &bucket_data) {
            let err_msg = "Downloaded bucket data failed single-hop verification";
            tracing::warn!("{} for bucket {}", err_msg, bucket_id);
            self.state
                .update_sync_status(
                    bucket_id,
                    BucketSyncStatus::Failed,
                    Some(err_msg.to_string()),
                )
                .await?;
            return Err(anyhow!(err_msg));
        }

        // 8. Download the pinset if it exists
        let pins_link = bucket_data.pins();
        tracing::info!(
            "Downloading pinset for bucket {} from peer {:?}",
            bucket_id,
            peer_addr
        );

        if let Err(e) = self
            .state
            .download_pinset_from_peer(pins_link, &peer_pub_key)
            .await
        {
            tracing::error!(
                "Failed to download pinset for bucket {} from peer {:?}: {}",
                bucket_id,
                peer_addr,
                e
            );
            // Don't fail the whole operation
        }

        // 9. Update bucket with new link and mark as synced
        tracing::info!(
            "Pull sync completed for bucket {}, updating to new link {:?}",
            bucket_id,
            new_link
        );

        self.state.update_bucket_link(bucket_id, new_link).await?;

        tracing::info!("Successfully synced bucket {} via pull", bucket_id);

        Ok(())
    }

    /// Push/announce a new bucket version to all peers
    ///
    /// This notifies all peers that we have a new version of the bucket.
    pub async fn push(&self, bucket_id: Uuid, new_link: Link) -> Result<()> {
        tracing::info!(
            "Push sync for bucket {} with new link {:?}",
            bucket_id,
            new_link
        );

        // 1. Get the list of peers for this bucket
        let peers = self.get_peers_for_bucket(bucket_id).await?;
        if peers.is_empty() {
            tracing::info!("No peers to announce to for bucket {}", bucket_id);
            return Ok(());
        }

        tracing::info!(
            "Announcing new bucket version to {} peers for bucket {}",
            peers.len(),
            bucket_id
        );

        // 2. Load the bucket data to get the previous link
        let bucket_data = self.state.load_bucket_data(&new_link).await?;
        let previous_link = bucket_data.previous().clone();

        // 3. Send announce messages to all peers in parallel
        let announce_futures: Vec<_> = peers
            .iter()
            .map(|peer_addr| {
                let peer = peer_addr.clone();
                let link = new_link.clone();
                let prev = previous_link.clone();
                let endpoint = self.endpoint.clone();
                async move {
                    match announce_to_peer(&endpoint, &peer, bucket_id, link, prev).await {
                        Ok(()) => {
                            tracing::debug!("Successfully announced to peer {:?}", peer);
                            Some(())
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to announce to peer {:?} for bucket {}: {}",
                                peer,
                                bucket_id,
                                e
                            );
                            None
                        }
                    }
                }
            })
            .collect();

        let results = join_all(announce_futures).await;

        // Count successful announcements
        let successful = results.iter().filter(|r| r.is_some()).count();
        let total = peers.len();

        tracing::info!(
            "Announced to {}/{} peers for bucket {}",
            successful,
            total,
            bucket_id
        );

        Ok(())
    }

    /// Handle a peer announce message
    ///
    /// This is called by the JAX protocol handler when a peer announces
    /// a new version of a bucket. It verifies the announce and pulls
    /// the new version if valid.
    pub async fn handle_peer_announce(
        &self,
        bucket_id: Uuid,
        peer_id: String,
        new_link: Link,
        previous_link: Option<Link>,
    ) -> Result<()> {
        tracing::info!(
            "Handling peer announce from {} for bucket {} with new link {:?}",
            peer_id,
            bucket_id,
            new_link
        );

        // Parse peer public key from peer_id (hex string)
        let peer_pub_key = PublicKey::from_hex(&peer_id)
            .map_err(|e| anyhow!("Invalid peer public key {}: {}", peer_id, e))?;

        // 1. Get bucket from state
        let bucket = match self.state.get_bucket(bucket_id).await? {
            Some(b) => b,
            None => {
                // Bucket doesn't exist, create it from peer announce
                return self
                    .create_bucket_from_announce(bucket_id, peer_pub_key, new_link)
                    .await;
            }
        };

        let current_link = bucket.link.clone();
        let our_previous_link = bucket.previous_link.clone();

        // 2. Verify provenance: peer must be in bucket shares
        if !self.verify_provenance(bucket_id, &peer_pub_key).await? {
            let err_msg = format!("Peer {} not authorized for bucket {}", peer_id, bucket_id);
            tracing::warn!("{}", err_msg);
            self.state
                .update_sync_status(bucket_id, BucketSyncStatus::Failed, Some(err_msg))
                .await?;
            return Ok(());
        }

        // 3. Check if previous_link matches our current link (single-hop verification)
        if let Some(prev) = previous_link {
            if prev != current_link {
                let err_msg = "Single-hop verification failed: out of order update";
                tracing::warn!(
                    "{}: peer's previous {:?} != our current {:?}",
                    err_msg,
                    prev,
                    current_link
                );
                // if the previous link is the same as our previous link, it's a duplicate
                if prev == self.state.get_previous_link(bucket_id).await? {
                    tracing::warn!("this probably means the peer sent a duplicate announcement");
                    return Ok(());
                }
                self.state
                    .update_sync_status(
                        bucket_id,
                        BucketSyncStatus::Failed,
                        Some(err_msg.to_string()),
                    )
                    .await?;
                return Ok(());
            }
        } else {
            // No previous link means this is an initial version, not an update
            let err_msg = "Announce must include previous link for updates";
            tracing::warn!("{}", err_msg);
            self.state
                .update_sync_status(
                    bucket_id,
                    BucketSyncStatus::Failed,
                    Some(err_msg.to_string()),
                )
                .await?;
            return Ok(());
        }

        // 4. Download and verify the new BucketData
        let bucket_data = match self
            .state
            .download_bucket_from_peer(&new_link, &peer_pub_key)
            .await
        {
            Ok(data) => data,
            Err(e) => {
                tracing::error!(
                    "Failed to download bucket data from link {:?}: {}",
                    new_link,
                    e
                );
                self.state
                    .update_sync_status(
                        bucket_id,
                        BucketSyncStatus::Failed,
                        Some(format!("Failed to download new bucket data: {}", e)),
                    )
                    .await?;
                return Err(e);
            }
        };

        // 5. Double-check single-hop on the downloaded BucketData
        if !verify_single_hop(&current_link, &bucket_data) {
            let err_msg = "Downloaded bucket data failed single-hop verification";
            tracing::warn!("{} for bucket {}", err_msg, bucket_id);
            self.state
                .update_sync_status(
                    bucket_id,
                    BucketSyncStatus::Failed,
                    Some(err_msg.to_string()),
                )
                .await?;
            return Err(anyhow!(err_msg));
        }

        // 6. Download the pinset
        let pins_link = bucket_data.pins();
        tracing::info!(
            "Downloading pinset for bucket {} update from peer {}",
            bucket_id,
            peer_id
        );

        if let Err(e) = self
            .state
            .download_pinset_from_peer(pins_link, &peer_pub_key)
            .await
        {
            tracing::error!(
                "Failed to download pinset for bucket {} update from peer {}: {}",
                bucket_id,
                peer_id,
                e
            );
            // Don't fail the whole operation
        }

        // 7. All checks passed! Update bucket with new link and mark as synced
        tracing::info!(
            "Peer announce validated for bucket {}, updating to new link {:?}",
            bucket_id,
            new_link
        );

        self.state.update_bucket_link(bucket_id, new_link).await?;

        tracing::info!(
            "Successfully synced bucket {} from peer announce",
            bucket_id
        );

        Ok(())
    }

    // ===== Helper Methods =====

    /// Get list of peer NodeAddrs for a bucket (excluding ourselves)
    async fn get_peers_for_bucket(&self, bucket_id: Uuid) -> Result<Vec<NodeAddr>> {
        let shares = self.state.get_bucket_shares(bucket_id).await?;

        // Convert shares to NodeAddr, excluding ourselves
        let mut peers = Vec::new();
        for share in shares {
            if share.public_key == self.our_node_id {
                continue; // Skip ourselves
            }

            // Parse public key from hex
            match PublicKey::from_hex(&share.public_key) {
                Ok(pub_key) => {
                    peers.push(NodeAddr::new(*pub_key));
                }
                Err(e) => {
                    tracing::warn!(
                        "Invalid public key {} for bucket {}: {}",
                        share.public_key,
                        bucket_id,
                        e
                    );
                }
            }
        }

        Ok(peers)
    }

    /// Verify that a peer is in the bucket's shares (provenance check)
    async fn verify_provenance(&self, bucket_id: Uuid, peer_pub_key: &PublicKey) -> Result<bool> {
        let shares = self.state.get_bucket_shares(bucket_id).await?;
        let peer_hex = peer_pub_key.to_hex();

        Ok(shares.iter().any(|share| share.public_key == peer_hex))
    }

    /// Create a bucket from a peer announce (for new buckets we don't have)
    async fn create_bucket_from_announce(
        &self,
        bucket_id: Uuid,
        peer_pub_key: PublicKey,
        new_link: Link,
    ) -> Result<()> {
        tracing::info!(
            "Bucket {} not found, creating from peer announce",
            bucket_id
        );

        // Download the BucketData from the peer
        let bucket_data = self
            .state
            .download_bucket_from_peer(&new_link, &peer_pub_key)
            .await
            .map_err(|e| {
                anyhow!(
                    "Failed to download bucket data from peer for link {:?}: {}",
                    new_link,
                    e
                )
            })?;

        // Get the bucket name from the BucketData
        let bucket_name = bucket_data.name().to_string();

        // Create the bucket with the fetched data
        tracing::info!(
            "Creating bucket {} with name '{}' from peer announce",
            bucket_id,
            bucket_name
        );
        self.state
            .create_bucket(bucket_id, bucket_name, new_link.clone())
            .await?;

        // Download the pinset
        let pins_link = bucket_data.pins();
        tracing::info!("Downloading pinset for bucket {} from peer", bucket_id);

        if let Err(e) = self
            .state
            .download_pinset_from_peer(pins_link, &peer_pub_key)
            .await
        {
            tracing::error!(
                "Failed to download pinset for bucket {} from peer: {}",
                bucket_id,
                e
            );
            // Don't fail the whole operation, bucket is still created
        }

        tracing::info!(
            "Created bucket {} from peer announce with link {:?}",
            bucket_id,
            new_link
        );

        Ok(())
    }
}

// ===== Helper Functions =====

/// Verify single-hop: peer's previous must equal our current link
fn verify_single_hop(current_link: &Link, peer_bucket_data: &Manifest) -> bool {
    match peer_bucket_data.previous() {
        Some(prev) => prev == current_link,
        None => false, // No previous means it's initial version, not a single-hop update
    }
}

// ===== JaxCallback Implementation =====

/// Implement JaxCallback so the JAX protocol handler can call the sync manager directly
#[async_trait]
impl<S> JaxCallback for SyncManager<S>
where
    S: SyncStateProvider + 'static,
{
    async fn on_peer_announce(
        &self,
        bucket_id: Uuid,
        peer_id: String,
        new_link: Link,
        previous_link: Option<Link>,
    ) -> Result<()> {
        self.handle_peer_announce(bucket_id, peer_id, new_link, previous_link)
            .await
    }
}

impl<S> std::fmt::Debug for SyncManager<S>
where
    S: SyncStateProvider,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncManager")
            .field("our_node_id", &self.our_node_id)
            .finish()
    }
}
