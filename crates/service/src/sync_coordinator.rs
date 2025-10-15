use flume::Receiver;
use std::sync::Arc;
use uuid::Uuid;

use common::crypto::PublicKey;
use common::linked_data::Link;
use common::peer::{Peer, PeerStateProvider};

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

/// Minimal sync coordinator - just dispatches events to Peer methods
///
/// This replaces the large SyncManager with a simple event loop
/// that delegates all sync logic to the Peer.
pub struct SyncCoordinator {
    peer: Peer,
    state: Arc<dyn PeerStateProvider>,
}

impl SyncCoordinator {
    pub fn new(peer: Peer, state: Arc<dyn PeerStateProvider>) -> Self {
        Self { peer, state }
    }

    /// Run the sync event loop
    ///
    /// This processes sync events from the channel and dispatches them
    /// to the appropriate Peer sync methods.
    pub async fn run(self, receiver: Receiver<SyncEvent>) {
        tracing::info!("Sync coordinator started");

        while let Ok(event) = receiver.recv_async().await {
            tracing::debug!("Received sync event: {:?}", event);

            let result = match event {
                SyncEvent::Pull { bucket_id } => {
                    self.peer.sync_pull(bucket_id, self.state.clone()).await
                }

                SyncEvent::Push {
                    bucket_id,
                    new_link,
                } => {
                    self.peer
                        .sync_push(bucket_id, new_link, self.state.clone())
                        .await
                }

                SyncEvent::PeerAnnounce {
                    bucket_id,
                    peer_id,
                    new_link,
                    previous_link,
                } => {
                    // Parse peer public key from hex string
                    let peer_key = match PublicKey::from_hex(&peer_id) {
                        Ok(key) => key,
                        Err(e) => {
                            tracing::error!(
                                "Invalid peer public key {} for bucket {}: {}",
                                peer_id,
                                bucket_id,
                                e
                            );
                            continue;
                        }
                    };

                    self.peer
                        .sync_handle_announce(
                            bucket_id,
                            peer_key,
                            new_link,
                            previous_link,
                            self.state.clone(),
                        )
                        .await
                }

                SyncEvent::Retry { bucket_id } => {
                    self.peer.sync_pull(bucket_id, self.state.clone()).await
                }
            };

            if let Err(e) = result {
                tracing::error!("Sync error: {}", e);
            }
        }

        tracing::info!("Sync coordinator stopped");
    }
}
