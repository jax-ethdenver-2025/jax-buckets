use std::sync::Arc;

use futures::future::join_all;
use iroh::Endpoint;
use uuid::Uuid;

use crate::bucket::Manifest;
use crate::crypto::PublicKey;
use crate::linked_data::{BlockEncoded, Link};

use super::{
    announce_to_peer, fetch_bucket, ping_peer, BlobsStore, BucketSyncStatus, NodeAddr, Peer,
    PeerStateProvider, SyncStatus,
};

/// Maximum depth to traverse when verifying multi-hop chains
const MAX_HISTORY_DEPTH: usize = 100;

/// Result of multi-hop verification when walking a peer's chain
enum MultiHopOutcome {
    /// Found a manifest whose previous equals our current link
    Verified { depth: usize },
    /// Chain terminated without including our current link
    Fork,
    /// Walk exceeded the configured maximum depth
    DepthExceeded,
}

/// Handle an announced update from a peer (standalone function for JAX protocol handler)
///
/// This verifies the peer's provenance, verifies the chain,
/// and applies the update if valid.
pub async fn handle_announce(
    bucket_id: Uuid,
    peer_id: PublicKey,
    new_link: Link,
    _previous_link: Option<Link>,
    state: Arc<dyn PeerStateProvider>,
) -> anyhow::Result<()> {
    let peer_label = peer_id.to_hex();
    let blobs = state.blobs();
    let endpoint = state.endpoint();

    // 1. Check if bucket exists
    let current_link = match state.get_bucket_link(bucket_id).await? {
        Some(link) => link,
        None => {
            tracing::info!(
                "Bucket {} not found, will create from peer announce",
                bucket_id
            );
            // Create bucket from peer
            create_bucket_from_peer(bucket_id, &new_link, &peer_id, &peer_label, blobs, endpoint, &state)
                .await?;
            return Ok(());
        }
    };

    // 2. Verify provenance: peer must be in bucket shares
    match verify_provenance(bucket_id, &peer_id, &state).await {
        Ok(true) => {
            tracing::debug!(
                "Provenance verified for peer {} on bucket {}",
                peer_label,
                bucket_id
            );
        }
        Ok(false) => {
            tracing::warn!(
                "Provenance check failed: peer {} not in shares for bucket {}",
                peer_label,
                bucket_id
            );
            return Err(anyhow::anyhow!(
                "Peer {} not authorized for this bucket",
                peer_label
            ));
        }
        Err(e) => {
            tracing::error!("Error verifying provenance: {}", e);
            return Err(e);
        }
    }

    // 3. Verify and apply update
    verify_and_apply_update(
        bucket_id,
        &current_link,
        &new_link,
        &peer_id,
        &peer_label,
        blobs,
        endpoint,
        &state,
    )
    .await
}

/// Verify that a peer is in the bucket's shares (provenance check)
async fn verify_provenance(
    bucket_id: Uuid,
    peer_pub_key: &PublicKey,
    state: &Arc<dyn PeerStateProvider>,
) -> anyhow::Result<bool> {
    let shares = state.get_bucket_shares(bucket_id).await?;
    let peer_hex = peer_pub_key.to_hex();

    Ok(shares.iter().any(|share| share.public_key == peer_hex))
}

/// Download BucketData from a specific peer
async fn download_from_peer(
    link: &Link,
    peer_node_id: &PublicKey,
    blobs: &BlobsStore,
    endpoint: &Endpoint,
) -> anyhow::Result<Manifest> {
    let hash = *link.hash();

    // Download from the specific peer
    let peer_ids = vec![(*peer_node_id).into()];
    blobs.download_hash(hash, peer_ids, endpoint).await?;

    // Now get it from local store
    let data = blobs.get(&hash).await?;
    let bucket_data = Manifest::decode(&data)?;

    Ok(bucket_data)
}

/// Iteratively verify that a peer's latest link chains back to our current link.
async fn verify_multi_hop(
    peer_pub_key: &PublicKey,
    latest_link: &Link,
    our_current_link: &Link,
    first_manifest: Option<Manifest>,
    blobs: &BlobsStore,
    endpoint: &Endpoint,
) -> anyhow::Result<MultiHopOutcome> {
    let mut cursor = latest_link.clone();
    let mut cached = first_manifest;

    for depth in 0..MAX_HISTORY_DEPTH {
        // Download or reuse the manifest at the current cursor from the specific peer
        let manifest = match cached.take() {
            Some(m) => m,
            None => match download_from_peer(&cursor, peer_pub_key, blobs, endpoint).await {
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
/// download the pinset, and update the bucket link.
async fn verify_and_apply_update(
    bucket_id: Uuid,
    current_link: &Link,
    new_link: &Link,
    peer_pub_key: &PublicKey,
    peer_label: &str,
    blobs: &BlobsStore,
    endpoint: &Endpoint,
    state: &Arc<dyn PeerStateProvider>,
) -> anyhow::Result<()> {
    // 1) Download the latest manifest (cache for verification)
    let bucket_data = match download_from_peer(new_link, peer_pub_key, blobs, endpoint).await {
        Ok(data) => data,
        Err(e) => {
            tracing::error!(
                "Failed to download bucket data from peer {} for link {:?}: {}",
                peer_label,
                new_link,
                e
            );
            return Err(e);
        }
    };

    // 2) Multi-hop verify the chain from latest back to our current link
    match verify_multi_hop(
        peer_pub_key,
        new_link,
        current_link,
        Some(bucket_data.clone()),
        blobs,
        endpoint,
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
            return Err(anyhow::anyhow!(
                "Multi-hop verification failed: chain mismatch or fork"
            ));
        }
        Ok(MultiHopOutcome::DepthExceeded) => {
            tracing::error!(
                "Multi-hop verification failed (depth exceeded) for bucket {}",
                bucket_id
            );
            return Err(anyhow::anyhow!(
                "Multi-hop verification failed: depth exceeded"
            ));
        }
        Err(e) => {
            tracing::error!(
                "Error during multi-hop verification for bucket {}: {}",
                bucket_id,
                e
            );
            return Err(e);
        }
    }

    // 3) Download the pinset for the verified latest
    let pins_link = bucket_data.pins();
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
            tracing::warn!(
                "Failed to download pinset for bucket {} from peer {}: {}",
                bucket_id,
                peer_label,
                e
            );
            // Do not fail the overall operation on pinset errors
        }
    }

    // 4) Update the bucket's link and mark as synced
    state
        .update_bucket_link_and_sync(bucket_id, new_link.clone())
        .await?;

    tracing::info!(
        "Successfully applied update for bucket {} from peer {}",
        bucket_id,
        peer_label
    );

    Ok(())
}

/// Create a new local bucket entry from a peer's announced link.
async fn create_bucket_from_peer(
    bucket_id: Uuid,
    new_link: &Link,
    peer_pub_key: &PublicKey,
    peer_label: &str,
    blobs: &BlobsStore,
    endpoint: &Endpoint,
    state: &Arc<dyn PeerStateProvider>,
) -> anyhow::Result<()> {
    // Download manifest to obtain bucket name
    let bucket_data = match download_from_peer(new_link, peer_pub_key, blobs, endpoint).await {
        Ok(data) => data,
        Err(e) => {
            tracing::error!(
                "Failed to download bucket data from peer {} for link {:?}: {}",
                peer_label,
                new_link,
                e
            );
            return Err(e);
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
    state
        .create_bucket(bucket_id, bucket_name, new_link.clone())
        .await?;

    // Best-effort pinset download
    let pins_link = bucket_data.pins();
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
            tracing::warn!(
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

impl Peer {
    /// Get list of peer NodeAddrs for a bucket (excluding ourselves)
    async fn get_peers_for_bucket(
        &self,
        bucket_id: Uuid,
        state: &Arc<dyn PeerStateProvider>,
    ) -> anyhow::Result<Vec<NodeAddr>> {
        // Get bucket shares from state
        let shares = state.get_bucket_shares(bucket_id).await?;

        // Get our node ID to filter ourselves out
        let our_node_id_hex = self.id().to_string();

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
        state: &Arc<dyn PeerStateProvider>,
    ) -> anyhow::Result<bool> {
        let shares = state.get_bucket_shares(bucket_id).await?;
        let peer_hex = peer_pub_key.to_hex();

        Ok(shares.iter().any(|share| share.public_key == peer_hex))
    }

    /// Download BucketData from a specific peer
    async fn download_from_peer(
        &self,
        link: &Link,
        peer_node_id: &PublicKey,
    ) -> anyhow::Result<Manifest> {
        let blobs = self.blobs();
        let endpoint = self.endpoint();
        let hash = *link.hash();

        // Download from the specific peer
        let peer_ids = vec![(*peer_node_id).into()];
        blobs.download_hash(hash, peer_ids, endpoint).await?;

        // Now get it from local store
        let data = blobs.get(&hash).await?;
        let bucket_data = Manifest::decode(&data)?;

        Ok(bucket_data)
    }

    /// Iteratively verify that a peer's latest link chains back to our current link.
    ///
    /// Walks the manifest chain from `latest_link` backwards by following `previous`,
    /// downloading only manifests from the specified peer, until it finds a manifest
    /// whose `previous` equals `our_current_link`. Returns the outcome of the verification.
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
    /// download the pinset, and update the bucket link.
    async fn verify_and_apply_update(
        &self,
        bucket_id: Uuid,
        current_link: &Link,
        new_link: &Link,
        peer_pub_key: &PublicKey,
        peer_label: &str,
        state: &Arc<dyn PeerStateProvider>,
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
                return Err(e);
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
                return Err(anyhow::anyhow!(
                    "Multi-hop verification failed: chain mismatch or fork"
                ));
            }
            Ok(MultiHopOutcome::DepthExceeded) => {
                tracing::error!(
                    "Multi-hop verification failed (depth exceeded) for bucket {}",
                    bucket_id
                );
                return Err(anyhow::anyhow!(
                    "Multi-hop verification failed: depth exceeded"
                ));
            }
            Err(e) => {
                tracing::error!(
                    "Error during multi-hop verification for bucket {}: {}",
                    bucket_id,
                    e
                );
                return Err(e);
            }
        }

        // 3) Download the pinset for the verified latest
        let pins_link = bucket_data.pins();
        let blobs = self.blobs();
        let endpoint = self.endpoint();
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
                tracing::warn!(
                    "Failed to download pinset for bucket {} from peer {}: {}",
                    bucket_id,
                    peer_label,
                    e
                );
                // Do not fail the overall operation on pinset errors
            }
        }

        // 4) Update the bucket's link and mark as synced
        state
            .update_bucket_link_and_sync(bucket_id, new_link.clone())
            .await?;

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
        state: &Arc<dyn PeerStateProvider>,
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
                return Err(e);
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
        state
            .create_bucket(bucket_id, bucket_name, new_link.clone())
            .await?;

        // Best-effort pinset download
        let pins_link = bucket_data.pins();
        let blobs = self.blobs();
        let endpoint = self.endpoint();
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
                tracing::warn!(
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

    /// Sync a bucket by pulling from peers
    ///
    /// This checks all peers for the bucket, finds one that's ahead of us,
    /// and downloads the latest version.
    pub async fn sync_pull(
        &self,
        bucket_id: Uuid,
        state: Arc<dyn PeerStateProvider>,
    ) -> anyhow::Result<()> {
        // 1. Get current bucket link
        let current_link = match state.get_bucket_link(bucket_id).await? {
            Some(link) => link,
            None => {
                tracing::warn!("Bucket {} not found for pull sync", bucket_id);
                return Ok(());
            }
        };

        // Set sync status to Syncing
        if let Err(e) = state
            .update_sync_status(bucket_id, BucketSyncStatus::Syncing, None)
            .await
        {
            tracing::warn!("Failed to update sync status to Syncing: {}", e);
        }

        // 2. Get list of peers for this bucket
        let peers = self.get_peers_for_bucket(bucket_id, &state).await?;
        if peers.is_empty() {
            tracing::info!("No peers found for bucket {}", bucket_id);
            // Mark as synced since we checked
            if let Err(e) = state
                .update_sync_status(bucket_id, BucketSyncStatus::Synced, None)
                .await
            {
                tracing::warn!("Failed to update sync status to Synced: {}", e);
            }
            return Ok(());
        }

        tracing::info!(
            "Pull sync: checking {} peers for bucket {}",
            peers.len(),
            bucket_id
        );

        // 3. Ping all peers in parallel to check sync status
        let endpoint = self.endpoint();
        let ping_futures: Vec<_> = peers
            .iter()
            .map(|peer_addr| {
                let peer = peer_addr.clone();
                let link = current_link.clone();
                async move {
                    match ping_peer(endpoint, &peer, bucket_id, link).await {
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
                // Mark as synced since we're up to date
                if let Err(e) = state
                    .update_sync_status(bucket_id, BucketSyncStatus::Synced, None)
                    .await
                {
                    tracing::warn!("Failed to update sync status to Synced: {}", e);
                }
                return Ok(());
            }
        };

        tracing::info!("Found ahead peer {:?} for bucket {}", peer_addr, bucket_id);

        // 5. Fetch the current bucket link from the ahead peer
        let new_link = match fetch_bucket(endpoint, &peer_addr, bucket_id).await {
            Ok(Some(link)) => link,
            Ok(None) => {
                tracing::warn!(
                    "Ahead peer {:?} returned no link for bucket {}",
                    peer_addr,
                    bucket_id
                );
                return Err(anyhow::anyhow!(
                    "Peer reported as ahead but has no bucket link"
                ));
            }
            Err(e) => {
                tracing::error!(
                    "Failed to fetch bucket link from peer {:?}: {}",
                    peer_addr,
                    e
                );
                return Err(e);
            }
        };

        tracing::info!(
            "Fetched new link {:?} from ahead peer {:?} for bucket {}",
            new_link,
            peer_addr,
            bucket_id
        );

        // 6. Verify and apply update
        let peer_pub_key = PublicKey::from(peer_addr.node_id);
        let result = self
            .verify_and_apply_update(
                bucket_id,
                &current_link,
                &new_link,
                &peer_pub_key,
                &format!("{:?}", peer_addr),
                &state,
            )
            .await;

        // Update sync status based on result
        match &result {
            Ok(_) => {
                // update_link_and_sync already set status to Synced
            }
            Err(e) => {
                // Mark as failed with error message
                if let Err(status_err) = state
                    .update_sync_status(bucket_id, BucketSyncStatus::Failed, Some(e.to_string()))
                    .await
                {
                    tracing::warn!("Failed to update sync status to Failed: {}", status_err);
                }
            }
        }

        result
    }

    /// Announce a new bucket version to all peers
    ///
    /// This sends announce messages to all peers for the bucket,
    /// informing them of the new version.
    pub async fn sync_push(
        &self,
        bucket_id: Uuid,
        new_link: Link,
        state: Arc<dyn PeerStateProvider>,
    ) -> anyhow::Result<()> {
        // 1. Get the list of peers for this bucket
        let peers = self.get_peers_for_bucket(bucket_id, &state).await?;
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
        let blobs = self.blobs();
        let data = blobs.get(new_link.hash()).await?;
        let bucket_data = Manifest::decode(&data)?;
        let previous_link = bucket_data.previous().clone();

        // 3. Send announce messages to all peers in parallel
        let endpoint = self.endpoint();
        let announce_futures: Vec<_> = peers
            .iter()
            .map(|peer_addr| {
                let peer = peer_addr.clone();
                let link = new_link.clone();
                let prev = previous_link.clone();
                async move {
                    match announce_to_peer(endpoint, &peer, bucket_id, link, prev).await {
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

    /// Handle an announced update from a peer
    ///
    /// This verifies the peer's provenance, verifies the chain,
    /// and applies the update if valid.
    pub async fn sync_handle_announce(
        &self,
        bucket_id: Uuid,
        peer_id: PublicKey,
        new_link: Link,
        _previous_link: Option<Link>,
        state: Arc<dyn PeerStateProvider>,
    ) -> anyhow::Result<()> {
        let peer_label = peer_id.to_hex();

        // 1. Check if bucket exists
        let current_link = match state.get_bucket_link(bucket_id).await? {
            Some(link) => link,
            None => {
                tracing::info!(
                    "Bucket {} not found, will create from peer announce",
                    bucket_id
                );
                // Create bucket from peer
                self.create_bucket_from_peer(bucket_id, &new_link, &peer_id, &peer_label, &state)
                    .await?;
                return Ok(());
            }
        };

        // 2. Verify provenance: peer must be in bucket shares
        match self.verify_provenance(bucket_id, &peer_id, &state).await {
            Ok(true) => {
                tracing::debug!(
                    "Provenance verified for peer {} on bucket {}",
                    peer_label,
                    bucket_id
                );
            }
            Ok(false) => {
                tracing::warn!(
                    "Provenance check failed: peer {} not in shares for bucket {}",
                    peer_label,
                    bucket_id
                );
                return Err(anyhow::anyhow!(
                    "Peer {} not authorized for this bucket",
                    peer_label
                ));
            }
            Err(e) => {
                tracing::error!("Error verifying provenance: {}", e);
                return Err(e);
            }
        }

        // 3. Verify and apply update
        self.verify_and_apply_update(
            bucket_id,
            &current_link,
            &new_link,
            &peer_id,
            &peer_label,
            &state,
        )
        .await
    }
}
