use flume::{Receiver, Sender};
use futures::future::join_all;
use std::sync::Arc;
use uuid::Uuid;

use crate::database::models::{Bucket, SyncStatus};
use crate::jax_state::MAX_HISTORY_DEPTH;
use crate::mount_ops;
use crate::ServiceState;
use common::bucket::Manifest;
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

/// Result of multi-hop verification when walking a peer's chain
enum MultiHopOutcome {
    /// Found a manifest whose previous equals our current link
    Verified { depth: usize },
    /// Chain terminated without including our current link
    Fork,
    /// Walk exceeded the configured maximum depth
    DepthExceeded,
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

    /// Iteratively verify that a peer's latest link chains back to our current link.
    ///
    /// Walks the manifest chain from `latest_link` backwards by following `previous`,
    /// downloading only manifests from the specified peer, until it finds a manifest
    /// whose `previous` equals `our_current_link`. Returns true if such a link is
    /// found within `MAX_MULTI_HOP_DEPTH`; returns false on fork/mismatch or when
    /// the chain terminates without reaching our current link.
    async fn verify_multi_hop(
        &self,
        peer_pub_key: &PublicKey,
        latest_link: &Link,
        our_current_link: &Link,
        first_manifest: Option<Manifest>,
    ) -> anyhow::Result<MultiHopOutcome> {
        let mut cursor = latest_link.clone();
        let mut cached = first_manifest;

        for depth in 0..MAX_HISTORY_DEPTH {
            // Download or reuse the manifest at the current cursor from the specific peer
            let manifest = match cached.take() {
                Some(m) => m,
                None => match self.download_from_peer(&cursor, peer_pub_key).await {
                    Ok(m) => m,
                    Err(e) => return Err(e),
                },
            };

            match manifest.previous() {
                Some(prev) if prev == our_current_link => {
                    return Ok(MultiHopOutcome::Verified { depth })
                }
                Some(prev) => {
                    // Continue walking backwards
                    cursor = prev.clone();
                }
                None => return Ok(MultiHopOutcome::Fork),
            }
        }

        Ok(MultiHopOutcome::DepthExceeded)
    }

    /// Download the peer's latest manifest, verify the chain back to our current link,
    /// download the pinset, and update the bucket link & sync status.
    async fn verify_and_apply_update(
        &self,
        bucket_id: Uuid,
        current_link: &Link,
        new_link: &Link,
        peer_pub_key: &PublicKey,
        peer_label: &str,
    ) -> anyhow::Result<()> {
        // 1) Download the latest manifest (cache for verification)
        let bucket_data = match self.download_from_peer(new_link, peer_pub_key).await {
            Ok(data) => data,
            Err(e) => {
                tracing::error!(
                    "Failed to download bucket data from peer {} for link {:?}: {}",
                    peer_label,
                    new_link,
                    e
                );
                if let Some(bucket) = Bucket::get_by_id(&bucket_id, self.state.database()).await? {
                    bucket
                        .update_sync_status(
                            SyncStatus::Failed,
                            Some(format!("Failed to download new bucket data: {}", e)),
                            self.state.database(),
                        )
                        .await?;
                }
                return Ok(());
            }
        };

        // 2) Multi-hop verify the chain from latest back to our current link
        match self
            .verify_multi_hop(
                peer_pub_key,
                new_link,
                current_link,
                Some(bucket_data.clone()),
            )
            .await
        {
            Ok(MultiHopOutcome::Verified { depth }) => {
                tracing::info!(
                    "Multi-hop verification succeeded for bucket {} from peer {} at depth {}",
                    bucket_id,
                    peer_label,
                    depth
                );
            }
            Ok(MultiHopOutcome::Fork) => {
                tracing::error!(
                    "Multi-hop verification failed (fork or mismatch) for bucket {}",
                    bucket_id
                );
                if let Some(bucket) = Bucket::get_by_id(&bucket_id, self.state.database()).await? {
                    bucket
                        .update_sync_status(
                            SyncStatus::Failed,
                            Some(
                                "Multi-hop verification failed: chain mismatch or fork".to_string(),
                            ),
                            self.state.database(),
                        )
                        .await?;
                }
                return Ok(());
            }
            Ok(MultiHopOutcome::DepthExceeded) => {
                tracing::error!(
                    "Multi-hop verification failed (depth exceeded) for bucket {}",
                    bucket_id
                );
                if let Some(bucket) = Bucket::get_by_id(&bucket_id, self.state.database()).await? {
                    bucket
                        .update_sync_status(
                            SyncStatus::Failed,
                            Some("Multi-hop verification failed: depth exceeded".to_string()),
                            self.state.database(),
                        )
                        .await?;
                }
                return Ok(());
            }
            Err(e) => {
                tracing::error!(
                    "Error during multi-hop verification for bucket {}: {}",
                    bucket_id,
                    e
                );
                if let Some(bucket) = Bucket::get_by_id(&bucket_id, self.state.database()).await? {
                    bucket
                        .update_sync_status(
                            SyncStatus::Failed,
                            Some(format!("Multi-hop verification error: {}", e)),
                            self.state.database(),
                        )
                        .await?;
                }
                return Ok(());
            }
        }

        // 3) Download the pinset for the verified latest
        let pins_link = bucket_data.pins();
        let blobs = self.state.node().blobs();
        let endpoint = self.state.node().endpoint();
        let pins_hash = *pins_link.hash();
        let peer_ids = vec![(*peer_pub_key).into()];

        match blobs
            .download_hash_list(pins_hash, peer_ids, endpoint)
            .await
        {
            Ok(()) => {
                tracing::info!(
                    "Successfully downloaded pinset for bucket {} from peer {}",
                    bucket_id,
                    peer_label
                );
            }
            Err(e) => {
                tracing::error!(
                    "Failed to download pinset for bucket {} from peer {}: {}",
                    bucket_id,
                    peer_label,
                    e
                );
                // Do not fail the overall operation on pinset errors
            }
        }

        // 4) Update the bucket's link and mark as synced
        if let Some(bucket) = Bucket::get_by_id(&bucket_id, self.state.database()).await? {
            bucket
                .update_link_and_sync(new_link.clone(), self.state.database())
                .await?;
        }

        tracing::info!(
            "Successfully applied update for bucket {} from peer {}",
            bucket_id,
            peer_label
        );

        Ok(())
    }

    /// Create a new local bucket entry from a peer's announced link.
    /// Downloads the manifest to obtain the bucket name, creates the DB row,
    /// and best-effort downloads the pinset.
    async fn create_bucket_from_peer(
        &self,
        bucket_id: Uuid,
        new_link: &Link,
        peer_pub_key: &PublicKey,
        peer_label: &str,
    ) -> anyhow::Result<()> {
        // Download manifest to obtain bucket name
        let bucket_data = match self.download_from_peer(new_link, peer_pub_key).await {
            Ok(data) => data,
            Err(e) => {
                tracing::error!(
                    "Failed to download bucket data from peer {} for link {:?}: {}",
                    peer_label,
                    new_link,
                    e
                );
                return Ok(());
            }
        };

        let bucket_name = bucket_data.name().to_string();

        // Create the bucket
        tracing::info!(
            "Creating bucket {} with name '{}' from peer {}",
            bucket_id,
            bucket_name,
            peer_label
        );
        Bucket::create(
            bucket_id,
            bucket_name,
            new_link.clone(),
            self.state.database(),
        )
        .await?;

        // Best-effort pinset download
        let pins_link = bucket_data.pins();
        let blobs = self.state.node().blobs();
        let endpoint = self.state.node().endpoint();
        let pins_hash = *pins_link.hash();
        let peer_ids = vec![(*peer_pub_key).into()];

        match blobs
            .download_hash_list(pins_hash, peer_ids, endpoint)
            .await
        {
            Ok(()) => {
                tracing::info!(
                    "Successfully downloaded pinset for bucket {} from peer {}",
                    bucket_id,
                    peer_label
                );
            }
            Err(e) => {
                tracing::error!(
                    "Failed to download pinset for bucket {} from peer {}: {}",
                    bucket_id,
                    peer_label,
                    e
                );
                // Do not fail the overall create on pinset errors
            }
        }

        tracing::info!(
            "Created bucket {} from peer {} with link {:?}",
            bucket_id,
            peer_label,
            new_link
        );

        Ok(())
    }

    /// get a bucket locally
    async fn get_bucket(&self, link: &Link) -> anyhow::Result<Manifest> {
        let data = self.state.node().blobs().get(link.hash()).await?;
        let bucket_data = Manifest::decode(&data)?;

        Ok(bucket_data)
    }

    /// Download BucketData from a specific peer
    async fn download_from_peer(
        &self,
        link: &Link,
        peer_node_id: &PublicKey,
    ) -> anyhow::Result<Manifest> {
        let blobs = self.state.node().blobs();
        let endpoint = self.state.node().endpoint();
        let hash = *link.hash();

        // Download from the specific peer
        let peer_ids = vec![(*peer_node_id).into()];
        blobs.download_hash(hash, peer_ids, endpoint).await?;

        // Now get it from local store
        let data = blobs.get(&hash).await?;
        let bucket_data = Manifest::decode(&data)?;

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

        let current_link: Link = bucket.link.into();
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
                    use tokio::time::{timeout, Duration};
                    match timeout(Duration::from_secs(2), ping_peer(endpoint, &peer, bucket_id, link)).await {
                        Ok(Ok(status)) => Some((peer, status)),
                        Ok(Err(e)) => { tracing::warn!("Failed to ping peer {:?}: {}", peer, e); None },
                        Err(_) => { tracing::warn!("Ping to peer {:?} timed out", peer); None },
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
        #[cfg(feature = "testkit")]
        let new_link = match tokio::time::timeout(
            std::time::Duration::from_secs(3),
            crate::testkit::protocol::test_fetch_bucket_addr(self.state.as_ref(), &peer_addr, bucket_id),
        )
        .await
        {
            Ok(Ok(Some(link))) => link,
            Ok(Ok(None)) => {
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
            Ok(Err(e)) => {
                tracing::error!("Failed to fetch bucket link from peer {:?}: {}", peer_addr, e);
                bucket
                    .update_sync_status(
                        SyncStatus::Failed,
                        Some(format!("Failed to fetch bucket link: {}", e)),
                        self.state.database(),
                    )
                    .await?;
                return Ok(());
            }
            Err(_) => {
                tracing::error!("Timed out fetching bucket link from peer {:?}", peer_addr);
                bucket
                    .update_sync_status(
                        SyncStatus::Failed,
                        Some("Timeout fetching bucket link".to_string()),
                        self.state.database(),
                    )
                    .await?;
                return Ok(());
            }
        };
        #[cfg(not(feature = "testkit"))]
        let new_link = match tokio::time::timeout(std::time::Duration::from_secs(3), fetch_bucket(endpoint, &peer_addr, bucket_id)).await {
            Ok(Ok(Some(link))) => link,
            Ok(Ok(None)) => {
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
            Ok(Err(e)) => {
                tracing::error!("Failed to fetch bucket link from peer {:?}: {}", peer_addr, e);
                bucket
                    .update_sync_status(
                        SyncStatus::Failed,
                        Some(format!("Failed to fetch bucket link: {}", e)),
                        self.state.database(),
                    )
                    .await?;
                return Ok(());
            }
            Err(_) => {
                tracing::error!("Timed out fetching bucket link from peer {:?}", peer_addr);
                bucket
                    .update_sync_status(
                        SyncStatus::Failed,
                        Some("Timeout fetching bucket link".to_string()),
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

        // Use shared verifier + applier
        let peer_pub_key = PublicKey::from(peer_addr.node_id);
        self.verify_and_apply_update(
            bucket_id,
            &current_link,
            &new_link,
            &peer_pub_key,
            &format!("{:?}", peer_addr),
        )
        .await
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
        #[cfg(feature = "testkit")]
        let announce_futures: Vec<_> = peers
            .iter()
            .map(|peer_addr| {
                let peer = peer_addr.clone();
                let link = new_link.clone();
                let prev = previous_link.clone();
                async move {
                    use tokio::time::{timeout, Duration};
                    match timeout(
                        Duration::from_secs(2),
                        crate::testkit::protocol::test_announce_to_peer_addr(
                            self.state.as_ref(),
                            &peer,
                            bucket_id,
                            link,
                            prev,
                        ),
                    )
                    .await
                    {
                        Ok(Ok(())) => {
                            tracing::debug!("Successfully announced to peer {:?}", peer);
                            Some(())
                        }
                        Ok(Err(e)) => {
                            tracing::warn!(
                                "Failed to announce to peer {:?} for bucket {}: {}",
                                peer,
                                bucket_id,
                                e
                            );
                            None
                        }
                        Err(_) => {
                            tracing::warn!("Announce to peer {:?} timed out", peer);
                            None
                        }
                    }
                }
            })
            .collect();
        #[cfg(not(feature = "testkit"))]
        let announce_futures: Vec<_> = peers
            .iter()
            .map(|peer_addr| {
                let peer = peer_addr.clone();
                let link = new_link.clone();
                let prev = previous_link.clone();
                async move {
                    use tokio::time::{timeout, Duration};
                    match timeout(Duration::from_secs(2), announce_to_peer(endpoint, &peer, bucket_id, link, prev)).await {
                        Ok(Ok(())) => { tracing::debug!("Successfully announced to peer {:?}", peer); Some(()) }
                        Ok(Err(e)) => { tracing::warn!("Failed to announce to peer {:?} for bucket {}: {}", peer, bucket_id, e); None }
                        Err(_) => { tracing::warn!("Announce to peer {:?} timed out", peer); None }
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
        _previous_link: Option<Link>,
    ) -> anyhow::Result<()> {
        // 1. Get bucket from database
        let bucket = match Bucket::get_by_id(&bucket_id, self.state.database()).await? {
            Some(b) => b,
            None => {
                tracing::info!(
                    "Bucket {} not found, will create from peer announce",
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

                // Use shared create path
                self.create_bucket_from_peer(bucket_id, &new_link, &peer_pub_key, &peer_id)
                    .await?;
                return Ok(());
            }
        };

        let current_link: Link = bucket.link.into();

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
        // Use shared verifier + applier
        self.verify_and_apply_update(bucket_id, &current_link, &new_link, &peer_pub_key, &peer_id)
            .await
    }
}
