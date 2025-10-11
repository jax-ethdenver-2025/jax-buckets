use flume::{Receiver, Sender};
use futures::future::join_all;
use std::sync::Arc;
use uuid::Uuid;

use crate::database::models::{Bucket, SyncStatus};
use crate::mount_ops;
use crate::ServiceState;
use common::bucket::BucketData;
use common::crypto::PublicKey;
use common::linked_data::{BlockEncoded, Link};
use common::peer::{
    announce_to_peer, fetch_bucket, ping_peer, NodeAddr, SyncStatus as PeerSyncStatus,
};

/// Events that trigger sync operations
#[derive(Debug, Clone)]
pub enum SyncEvent {
    /// Pull from peers when we're behind or out of sync
    Pull { bucket_id: Uuid },

    /// Push/announce to peers when we're ahead
    Push { bucket_id: Uuid, new_link: Link },

    /// Peer announced a new version
    PeerAnnounce {
        bucket_id: Uuid,
        peer_id: String,
        new_link: Link,
        previous_link: Option<Link>,
    },

    /// Retry a failed sync
    Retry { bucket_id: Uuid },
}

/// Sync manager handles bucket synchronization in the background
#[derive(Clone)]
pub struct SyncManager {
    sender: Sender<SyncEvent>,
    state: Arc<ServiceState>,
}

impl SyncManager {
    /// Create a new sync manager
    pub fn new(state: Arc<ServiceState>) -> (Self, Receiver<SyncEvent>) {
        let (sender, receiver) = flume::unbounded();

        let manager = Self { sender, state };

        (manager, receiver)
    }

    /// Get a clone of the sender for wiring into ServiceState
    pub fn sender(&self) -> Sender<SyncEvent> {
        self.sender.clone()
    }

    /// Run the sync event loop
    pub async fn run(self, receiver: Receiver<SyncEvent>) {
        tracing::info!("Sync manager started");

        while let Ok(event) = receiver.recv_async().await {
            tracing::debug!("Received sync event: {:?}", event);

            if let Err(e) = self.handle_event(event).await {
                tracing::error!("Error handling sync event: {}", e);
            }
        }

        tracing::info!("Sync manager stopped");
    }

    // ===== Helper Methods =====

    /// Get list of peer NodeAddrs for a bucket (excluding ourselves)
    async fn get_peers_for_bucket(&self, bucket_id: Uuid) -> anyhow::Result<Vec<NodeAddr>> {
        // Get bucket shares using mount_ops
        let shares = mount_ops::get_bucket_shares(bucket_id, &self.state).await?;

        // Get our node ID to filter ourselves out
        let our_node_id_hex = self.state.node().id().to_string();

        // Convert shares to NodeAddr, excluding ourselves
        let mut peers = Vec::new();
        for share in shares {
            if share.public_key == our_node_id_hex {
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
    async fn verify_provenance(
        &self,
        bucket_id: Uuid,
        peer_pub_key: &PublicKey,
    ) -> anyhow::Result<bool> {
        let shares = mount_ops::get_bucket_shares(bucket_id, &self.state).await?;
        let peer_hex = peer_pub_key.to_hex();

        Ok(shares.iter().any(|share| share.public_key == peer_hex))
    }

    /// Verify single-hop: peer's previous must equal our current link
    fn verify_single_hop(current_link: &Link, peer_bucket_data: &BucketData) -> bool {
        match peer_bucket_data.previous() {
            Some(prev) => prev == current_link,
            None => false, // No previous means it's initial version, not a single-hop update
        }
    }

    /// get a bucket locally
    async fn get_bucket(&self, link: &Link) -> anyhow::Result<BucketData> {
        let data = self.state.node().blobs().get(&link.hash()).await?;
        let bucket_data = BucketData::decode(&data)?;

        Ok(bucket_data)
    }

    /// Download BucketData from a specific peer
    async fn download_from_peer(
        &self,
        link: &Link,
        peer_node_id: &PublicKey,
    ) -> anyhow::Result<BucketData> {
        let blobs = self.state.node().blobs();
        let endpoint = self.state.node().endpoint();
        let hash = *link.hash();

        // Download from the specific peer
        let peer_ids = vec![(*peer_node_id).into()];
        blobs.download_hash(hash, peer_ids, endpoint).await?;

        // Now get it from local store
        let data = blobs.get(&hash).await?;
        let bucket_data = BucketData::decode(&data)?;

        Ok(bucket_data)
    }

    // ===== Event Handlers =====

    /// Handle a single sync event
    async fn handle_event(&self, event: SyncEvent) -> anyhow::Result<()> {
        match event {
            SyncEvent::Pull { bucket_id } => {
                tracing::info!("Handling pull sync for bucket {}", bucket_id);
                self.handle_pull(bucket_id).await
            }

            SyncEvent::Push {
                bucket_id,
                new_link,
            } => {
                tracing::info!(
                    "Handling push sync for bucket {} with new link {:?}",
                    bucket_id,
                    new_link
                );
                self.handle_push(bucket_id, new_link).await
            }

            SyncEvent::PeerAnnounce {
                bucket_id,
                peer_id,
                new_link,
                previous_link,
            } => {
                tracing::info!(
                    "Handling peer announce from {} for bucket {} with new link {:?}",
                    peer_id,
                    bucket_id,
                    new_link
                );
                self.handle_peer_announce(bucket_id, peer_id, new_link, previous_link)
                    .await
            }

            SyncEvent::Retry { bucket_id } => {
                tracing::info!("Retrying sync for bucket {}", bucket_id);
                self.handle_pull(bucket_id).await
            }
        }
    }

    /// Handle pull sync: download the latest bucket data from peers
    async fn handle_pull(&self, bucket_id: Uuid) -> anyhow::Result<()> {
        // 1. Get bucket from database
        let bucket = match Bucket::get_by_id(&bucket_id, self.state.database()).await? {
            Some(b) => b,
            None => {
                tracing::warn!("Bucket {} not found for pull sync", bucket_id);
                return Ok(());
            }
        };

        // Update sync status to Syncing
        bucket
            .update_sync_status(SyncStatus::Syncing, None, self.state.database())
            .await?;

        // 2. Get list of peers for this bucket
        let peers = self.get_peers_for_bucket(bucket_id).await?;
        if peers.is_empty() {
            tracing::info!("No peers found for bucket {}", bucket_id);
            bucket
                .update_sync_status(SyncStatus::Synced, None, self.state.database())
                .await?;
            return Ok(());
        }

        let current_link: Link = bucket.link.clone().into();
        tracing::info!(
            "Pull sync: checking {} peers for bucket {}",
            peers.len(),
            bucket_id
        );

        // 3. Ping all peers in parallel to check sync status
        let endpoint = self.state.node().endpoint();
        let ping_futures: Vec<_> = peers
            .iter()
            .map(|peer_addr| {
                let peer = peer_addr.clone();
                let link = current_link.clone();
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
            .find(|(_, status)| *status == PeerSyncStatus::Ahead);

        let (peer_addr, _) = match ahead_peer {
            Some(p) => p,
            None => {
                tracing::info!("No peers ahead of us for bucket {}", bucket_id);
                bucket
                    .update_sync_status(SyncStatus::Synced, None, self.state.database())
                    .await?;
                return Ok(());
            }
        };

        tracing::info!("Found ahead peer {:?} for bucket {}", peer_addr, bucket_id);

        // 5. Fetch the current bucket link from the ahead peer
        let new_link = match fetch_bucket(&endpoint, &peer_addr, bucket_id).await {
            Ok(Some(link)) => link,
            Ok(None) => {
                tracing::warn!(
                    "Ahead peer {:?} returned no link for bucket {}",
                    peer_addr,
                    bucket_id
                );
                bucket
                    .update_sync_status(
                        SyncStatus::OutOfSync,
                        Some("Peer reported as ahead but has no bucket link".to_string()),
                        self.state.database(),
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
                bucket
                    .update_sync_status(
                        SyncStatus::Failed,
                        Some(format!("Failed to fetch bucket link: {}", e)),
                        self.state.database(),
                    )
                    .await?;
                return Ok(());
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
        let bucket_data = match self.download_from_peer(&new_link, &peer_pub_key).await {
            Ok(data) => data,
            Err(e) => {
                tracing::error!(
                    "Failed to download bucket data from peer {:?} for link {:?}: {}",
                    peer_addr,
                    new_link,
                    e
                );
                bucket
                    .update_sync_status(
                        SyncStatus::Failed,
                        Some(format!("Failed to download new bucket data: {}", e)),
                        self.state.database(),
                    )
                    .await?;
                return Ok(());
            }
        };

        // 7. Verify single-hop: peer's previous must equal our current link
        if !Self::verify_single_hop(&current_link, &bucket_data) {
            tracing::warn!(
                "Single-hop verification failed during pull sync for bucket {}",
                bucket_id
            );
            bucket
                .update_sync_status(
                    SyncStatus::Failed,
                    Some("Downloaded bucket data failed single-hop verification".to_string()),
                    self.state.database(),
                )
                .await?;
            return Ok(());
        }

        // 8. Download the pinset if it exists
        if let Some(pins_link) = bucket_data.pins() {
            tracing::info!(
                "Downloading pinset for bucket {} from peer {:?}",
                bucket_id,
                peer_addr
            );
            let blobs = self.state.node().blobs();
            let pins_hash = *pins_link.hash();
            let peer_ids = vec![peer_pub_key.into()];

            match blobs
                .download_hash_list(pins_hash, peer_ids.clone(), &endpoint)
                .await
            {
                Ok(()) => {
                    tracing::info!(
                        "Successfully downloaded pinset for bucket {} from pull sync",
                        bucket_id
                    );

                    // Verify the pinset was downloaded
                    match blobs.stat(&pins_hash).await {
                        Ok(true) => {
                            tracing::debug!("Verified pinset hash {} exists locally", pins_hash)
                        }
                        Ok(false) => tracing::error!(
                            "Pinset hash {} NOT found locally after download!",
                            pins_hash
                        ),
                        Err(e) => {
                            tracing::error!("Error checking pinset hash {}: {}", pins_hash, e)
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to download pinset for bucket {} from peer {:?}: {}",
                        bucket_id,
                        peer_addr,
                        e
                    );
                    // Don't fail the whole operation
                }
            }
        } else {
            tracing::warn!(
                "BucketData for bucket {} has NO pinset link - bucket will be empty!",
                bucket_id
            );
        }

        // 9. Update bucket with new link and mark as synced
        tracing::info!(
            "Pull sync completed for bucket {}, updating to new link {:?}",
            bucket_id,
            new_link
        );

        bucket
            .update_link_and_sync(new_link, self.state.database())
            .await?;

        tracing::info!("Successfully synced bucket {} via pull", bucket_id);

        Ok(())
    }

    /// Handle push/announce: notify peers of our new version
    async fn handle_push(&self, bucket_id: Uuid, new_link: Link) -> anyhow::Result<()> {
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

        // 2. Download the BucketData to get the previous link
        let bucket_data = self.get_bucket(&new_link).await?;
        let previous_link = bucket_data.previous().clone();

        // 3. Send announce messages to all peers in parallel
        let endpoint = self.state.node().endpoint();
        let announce_futures: Vec<_> = peers
            .iter()
            .map(|peer_addr| {
                let peer = peer_addr.clone();
                let link = new_link.clone();
                let prev = previous_link.clone();
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

    /// Handle peer announce: verify and pull if valid
    async fn handle_peer_announce(
        &self,
        bucket_id: Uuid,
        peer_id: String,
        new_link: Link,
        previous_link: Option<Link>,
    ) -> anyhow::Result<()> {
        // 1. Get bucket from database
        let bucket = match Bucket::get_by_id(&bucket_id, self.state.database()).await? {
            Some(b) => b,
            None => {
                tracing::info!(
                    "Bucket {} not found, will create from peer announce after verification",
                    bucket_id
                );

                // Parse peer public key from peer_id (hex string)
                let peer_pub_key = match PublicKey::from_hex(&peer_id) {
                    Ok(key) => key,
                    Err(e) => {
                        tracing::error!(
                            "Invalid peer public key {} for bucket {}: {}",
                            peer_id,
                            bucket_id,
                            e
                        );
                        return Ok(());
                    }
                };

                // Download the BucketData from the peer
                let bucket_data = match self.download_from_peer(&new_link, &peer_pub_key).await {
                    Ok(data) => data,
                    Err(e) => {
                        tracing::error!(
                            "Failed to download bucket data from peer {} for link {:?}: {}",
                            peer_id,
                            new_link,
                            e
                        );
                        return Ok(());
                    }
                };

                // Get the bucket name from the BucketData
                let bucket_name = bucket_data.name().to_string();

                // Create the bucket with the fetched data
                tracing::info!(
                    "Creating bucket {} with name '{}' from peer announce",
                    bucket_id,
                    bucket_name
                );
                Bucket::create(
                    bucket_id,
                    bucket_name,
                    new_link.clone(),
                    self.state.database(),
                )
                .await?;

                // Download the pinset if it exists
                if let Some(pins_link) = bucket_data.pins() {
                    tracing::debug!("BucketData has pinset link: {:?}", pins_link);
                    tracing::info!(
                        "Downloading pinset for bucket {} from peer {}",
                        bucket_id,
                        peer_id
                    );
                    let blobs = self.state.node().blobs();
                    let endpoint = self.state.node().endpoint();
                    let pins_hash = *pins_link.hash();
                    let peer_ids = vec![peer_pub_key.into()];

                    tracing::debug!("Pinset hash: {:?}, peer_ids: {:?}", pins_hash, peer_ids);

                    match blobs
                        .download_hash_list(pins_hash, peer_ids.clone(), endpoint)
                        .await
                    {
                        Ok(()) => {
                            tracing::info!(
                                "Successfully downloaded pinset for bucket {}",
                                bucket_id
                            );

                            // Verify the pinset was actually downloaded
                            match blobs.stat(&pins_hash).await {
                                Ok(true) => tracing::debug!(
                                    "Verified pinset hash {} exists locally",
                                    pins_hash
                                ),
                                Ok(false) => tracing::error!(
                                    "Pinset hash {} NOT found locally after download!",
                                    pins_hash
                                ),
                                Err(e) => tracing::error!(
                                    "Error checking pinset hash {}: {}",
                                    pins_hash,
                                    e
                                ),
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to download pinset for bucket {} from peer {}: {}",
                                bucket_id,
                                peer_id,
                                e
                            );
                            // Don't fail the whole operation, bucket is still created
                        }
                    }
                } else {
                    tracing::warn!(
                        "BucketData for bucket {} has NO pinset link - bucket will be empty!",
                        bucket_id
                    );
                }

                tracing::info!(
                    "Created bucket {} from peer announce with link {:?}",
                    bucket_id,
                    new_link
                );
                return Ok(());
            }
        };

        let current_link: Link = bucket.link.clone().into();

        // 2. Parse peer public key from peer_id (hex string)
        let peer_pub_key = match PublicKey::from_hex(&peer_id) {
            Ok(key) => key,
            Err(e) => {
                tracing::error!(
                    "Invalid peer public key {} for bucket {}: {}",
                    peer_id,
                    bucket_id,
                    e
                );
                return Ok(());
            }
        };

        // 3. Verify provenance: peer must be in bucket shares
        match self.verify_provenance(bucket_id, &peer_pub_key).await {
            Ok(true) => {
                tracing::debug!(
                    "Provenance verified for peer {} on bucket {}",
                    peer_id,
                    bucket_id
                );
            }
            Ok(false) => {
                tracing::warn!(
                    "Provenance check failed: peer {} not in shares for bucket {}",
                    peer_id,
                    bucket_id
                );
                bucket
                    .update_sync_status(
                        SyncStatus::Failed,
                        Some(format!("Peer {} not authorized for this bucket", peer_id)),
                        self.state.database(),
                    )
                    .await?;
                return Ok(());
            }
            Err(e) => {
                tracing::error!("Error verifying provenance: {}", e);
                return Err(e);
            }
        }

        // 4. Check if previous_link matches our current link (single-hop verification)
        if let Some(prev) = previous_link {
            if prev != current_link {
                tracing::warn!(
                    "Single-hop check failed: peer's previous {:?} != our current {:?}",
                    prev,
                    current_link
                );
                bucket
                    .update_sync_status(
                        SyncStatus::Failed,
                        Some("Single-hop verification failed: out of order update".to_string()),
                        self.state.database(),
                    )
                    .await?;
                return Ok(());
            }
        } else {
            // No previous link means this is an initial version, not an update
            tracing::warn!("Peer announce has no previous link, rejecting");
            bucket
                .update_sync_status(
                    SyncStatus::Failed,
                    Some("Announce must include previous link for updates".to_string()),
                    self.state.database(),
                )
                .await?;
            return Ok(());
        }

        // 5. Download and verify the new BucketData
        let bucket_data = match self.download_from_peer(&new_link, &peer_pub_key).await {
            Ok(data) => data,
            Err(e) => {
                tracing::error!(
                    "Failed to download bucket data from link {:?}: {}",
                    new_link,
                    e
                );
                bucket
                    .update_sync_status(
                        SyncStatus::Failed,
                        Some(format!("Failed to download new bucket data: {}", e)),
                        self.state.database(),
                    )
                    .await?;
                return Ok(());
            }
        };

        // 6. Double-check single-hop on the downloaded BucketData
        if !Self::verify_single_hop(&current_link, &bucket_data) {
            tracing::warn!(
                "Single-hop verification failed on downloaded BucketData for bucket {}",
                bucket_id
            );
            bucket
                .update_sync_status(
                    SyncStatus::Failed,
                    Some("Downloaded bucket data failed single-hop verification".to_string()),
                    self.state.database(),
                )
                .await?;
            return Ok(());
        }

        // 7. All checks passed! Update bucket with new link and mark as synced
        tracing::info!(
            "Peer announce validated for bucket {}, updating to new link {:?}",
            bucket_id,
            new_link
        );

        // Download the pinset if it exists
        if let Some(pins_link) = bucket_data.pins() {
            tracing::debug!("BucketData has pinset link: {:?}", pins_link);
            tracing::info!(
                "Downloading pinset for bucket {} update from peer {}",
                bucket_id,
                peer_id
            );
            let blobs = self.state.node().blobs();
            let endpoint = self.state.node().endpoint();
            let pins_hash = *pins_link.hash();
            let peer_ids = vec![peer_pub_key.into()];

            tracing::debug!("Pinset hash: {:?}, peer_ids: {:?}", pins_hash, peer_ids);

            match blobs
                .download_hash_list(pins_hash, peer_ids.clone(), endpoint)
                .await
            {
                Ok(()) => {
                    tracing::info!(
                        "Successfully downloaded pinset for bucket {} update",
                        bucket_id
                    );

                    // Verify the pinset was actually downloaded
                    match blobs.stat(&pins_hash).await {
                        Ok(true) => {
                            tracing::debug!("Verified pinset hash {} exists locally", pins_hash)
                        }
                        Ok(false) => tracing::error!(
                            "Pinset hash {} NOT found locally after download!",
                            pins_hash
                        ),
                        Err(e) => {
                            tracing::error!("Error checking pinset hash {}: {}", pins_hash, e)
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to download pinset for bucket {} update from peer {}: {}",
                        bucket_id,
                        peer_id,
                        e
                    );
                    // Don't fail the whole operation, we'll mark it as partially synced
                }
            }
        } else {
            tracing::warn!(
                "BucketData for bucket {} update has NO pinset link - bucket will be empty!",
                bucket_id
            );
        }

        bucket
            .update_link_and_sync(new_link, self.state.database())
            .await?;

        tracing::info!(
            "Successfully synced bucket {} from peer announce",
            bucket_id
        );

        Ok(())
    }
}
