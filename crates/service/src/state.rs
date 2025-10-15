use std::sync::Arc;
use url::Url;

use super::config::Config;
use super::database::{Database, DatabaseSetupError};
use super::peer_state::ServicePeerState;
use super::sync_coordinator::SyncEvent;

use common::crypto::SecretKey;
use common::peer::{BlobsStore, Peer};

/// Main service state - orchestrates all components
#[derive(Clone)]
pub struct State {
    peer_state: Arc<ServicePeerState>,
    peer: Peer,
    sync_sender: flume::Sender<SyncEvent>,
}

impl State {
    pub async fn from_config(
        config: &Config,
        sync_sender: flume::Sender<SyncEvent>,
    ) -> Result<Self, StateSetupError> {
        // 1. Setup database
        let sqlite_database_url = match config.sqlite_path {
            Some(ref path) => {
                // check that the path exists
                if !path.exists() {
                    return Err(StateSetupError::DatabasePathDoesNotExist);
                }
                // parse the path into a URL
                Url::parse(&format!("sqlite://{}", path.display()))
                    .map_err(|_| StateSetupError::InvalidDatabaseUrl)
            }
            // otherwise just set up an in-memory database
            None => Url::parse("sqlite::memory:").map_err(|_| StateSetupError::InvalidDatabaseUrl),
        }?;
        tracing::info!("Database URL: {:?}", sqlite_database_url);
        let database = Database::connect(&sqlite_database_url).await?;

        // 2. Setup node secret
        let node_secret = config
            .node_secret
            .clone()
            .unwrap_or_else(SecretKey::generate);

        // 3. Setup blobs store
        let blobs_store_path = config.node_blobs_store_path.clone().unwrap_or_else(|| {
            let temp_dir = tempfile::tempdir().expect("failed to create temporary directory");
            temp_dir.path().to_path_buf()
        });
        tracing::debug!("ServiceState::from_config - loading blobs store");
        let blobs = BlobsStore::load(&blobs_store_path)
            .await
            .map_err(|e| StateSetupError::BlobsStoreError(e.to_string()))?;
        tracing::debug!("ServiceState::from_config - blobs store loaded successfully");

        // 4. Create peer state which will own endpoint creation
        let peer_state = Arc::new(ServicePeerState::from_config(
            database.clone(),
            blobs.clone(),
            blobs_store_path.clone(),
            node_secret.clone(),
            config.node_listen_addr,
        ).await.map_err(|e| StateSetupError::PeerStateError(e.to_string()))?);

        // 5. Build peer from the state (peer will use the endpoint from state via the protocol handler)
        let peer = Peer::from_state(
            peer_state.clone(),
            blobs_store_path,
        );

        // Log the bound addresses
        let bound_addrs = peer.endpoint().bound_sockets();
        tracing::info!("Node id: {} (with JAX protocol)", peer.id());
        tracing::info!("Peer listening on: {:?}", bound_addrs);

        Ok(Self {
            peer_state,
            peer,
            sync_sender,
        })
    }

    pub fn peer(&self) -> &Peer {
        &self.peer
    }

    pub fn node(&self) -> &Peer {
        // Alias for backwards compatibility
        &self.peer
    }

    pub fn peer_state(&self) -> &Arc<ServicePeerState> {
        &self.peer_state
    }

    pub fn database(&self) -> &Database {
        self.peer_state.database()
    }

    pub fn sync_sender(&self) -> &flume::Sender<SyncEvent> {
        &self.sync_sender
    }

    /// Send a sync event to the sync coordinator
    pub fn send_sync_event(&self, event: SyncEvent) -> Result<(), SyncEventError> {
        self.sync_sender
            .send(event)
            .map_err(|_| SyncEventError::SendFailed)
    }
}

impl AsRef<Peer> for State {
    fn as_ref(&self) -> &Peer {
        &self.peer
    }
}

impl AsRef<Database> for State {
    fn as_ref(&self) -> &Database {
        self.database()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StateSetupError {
    #[error("Database path does not exist")]
    DatabasePathDoesNotExist,
    #[error("Database setup error")]
    DatabaseSetupError(#[from] DatabaseSetupError),
    #[error("Invalid database URL")]
    InvalidDatabaseUrl,
    #[error("Blobs store error: {0}")]
    BlobsStoreError(String),
    #[error("Peer state error: {0}")]
    PeerStateError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum SyncEventError {
    #[error("Failed to send sync event")]
    SendFailed,
}
