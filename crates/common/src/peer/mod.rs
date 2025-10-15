use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;

use iroh::discovery::pkarr::dht::DhtDiscovery;
use iroh::{protocol::Router, Endpoint, NodeId};
use tokio::sync::watch::Receiver as WatchReceiver;

use crate::crypto::SecretKey;

mod blobs_store;
pub mod jax_protocol;
mod sync;

pub use blobs_store::{BlobsStore, BlobsStoreError};
pub use jax_protocol::{
    announce_to_peer, fetch_bucket, ping_peer, AnnounceCallback, BucketSyncStatus, JaxProtocol,
    PeerStateProvider, PingRequest, PingResponse, ShareInfo, SyncStatus, JAX_ALPN,
};

// Re-export iroh types for convenience
pub use iroh::NodeAddr;

#[derive(Clone, Default)]
pub struct PeerBuilder {
    /// the socket addr to expose the peer on
    ///  if not set, an ephemeral port will be used
    socket_addr: Option<SocketAddr>,
    /// the identity of the peer, as a SecretKey
    secret_key: Option<SecretKey>,
    // TODO (amiller68): i would like to just inject
    //  the blobs store, but I think I need it to build the
    //  router, so that's not possible yet
    /// the path to the blobs store on the peer's filesystem
    ///  if not set a temporary directory will be used
    blobs_store_path: Option<PathBuf>,
    /// pre-loaded blobs store (if provided, blobs_store_path is ignored)
    blobs_store: Option<BlobsStore>,
    /// optional state provider for the JAX protocol
    protocol_state: Option<std::sync::Arc<dyn PeerStateProvider>>,
    /// optional callback for announce messages
    announce_callback: Option<AnnounceCallback>,
}

// TODO (amiller68): proper errors
impl PeerBuilder {
    pub fn new() -> Self {
        PeerBuilder {
            socket_addr: None,
            secret_key: None,
            blobs_store_path: None,
            blobs_store: None,
            protocol_state: None,
            announce_callback: None,
        }
    }

    pub fn socket_addr(mut self, socket_addr: SocketAddr) -> Self {
        self.socket_addr = Some(socket_addr);
        self
    }

    pub fn secret_key(mut self, secret_key: SecretKey) -> Self {
        self.secret_key = Some(secret_key);
        self
    }

    pub fn blobs_store_path(mut self, path: PathBuf) -> Self {
        self.blobs_store_path = Some(path);
        self
    }

    pub fn blobs_store(mut self, blobs: BlobsStore) -> Self {
        self.blobs_store = Some(blobs);
        self
    }

    pub fn protocol_state(mut self, state: std::sync::Arc<dyn PeerStateProvider>) -> Self {
        self.protocol_state = Some(state);
        self
    }

    pub fn announce_callback(mut self, callback: AnnounceCallback) -> Self {
        self.announce_callback = Some(callback);
        self
    }

    pub async fn build(self) -> Peer {
        // set the socket port to unspecified if not set
        let socket_addr = self
            .socket_addr
            .unwrap_or_else(|| SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0));
        // generate a new secret key if not set
        let secret_key = self.secret_key.unwrap_or_else(SecretKey::generate);

        // Load or use provided blobs store
        let (blob_store, blobs_store_path) = if let Some(blobs) = self.blobs_store {
            tracing::debug!("PeerBuilder::build - using pre-loaded blobs store");
            // Use pre-loaded blobs store
            // Use the provided path, or a dummy one (path is only used for logging)
            let path = self
                .blobs_store_path
                .unwrap_or_else(|| PathBuf::from("/unknown"));
            (blobs, path)
        } else {
            tracing::debug!("PeerBuilder::build - loading blobs store from path");
            // Load from path
            let blobs_store_path = self.blobs_store_path.unwrap_or_else(|| {
                // Create a temporary directory for the blobs store
                let temp_dir = tempfile::tempdir().expect("failed to create temporary directory");
                temp_dir.path().to_path_buf()
            });
            let blob_store = BlobsStore::load(&blobs_store_path)
                .await
                .expect("failed to load blob store");
            (blob_store, blobs_store_path)
        };

        // now get to building

        // Convert the SocketAddr to a SocketAddrV4
        let addr = SocketAddrV4::new(
            socket_addr
                .ip()
                .to_string()
                .parse::<Ipv4Addr>()
                .expect("failed to parse IP address"),
            socket_addr.port(),
        );

        // setup our discovery mechanism for our peer
        let mainline_discovery = DhtDiscovery::builder()
            .secret_key(secret_key.0.clone())
            .build()
            .expect("failed to build mainline discovery");

        // Create the endpoint with our key and discovery
        let endpoint = Endpoint::builder()
            .secret_key(secret_key.0.clone())
            .discovery(mainline_discovery)
            .bind_addr_v4(addr)
            .bind()
            .await
            .expect("failed to bind ephemeral endpoint");

        Peer {
            blob_store,
            secret: secret_key,
            endpoint,
            blobs_store_path,
            protocol_state: self.protocol_state,
            announce_callback: self.announce_callback,
        }
    }
}

// TODO (amiller68): this can prolly be simpler /
//  idk if we need all of this, but it'll work for now
#[derive(Clone)]
pub struct Peer {
    blob_store: BlobsStore,
    secret: SecretKey,
    endpoint: Endpoint,
    blobs_store_path: PathBuf,
    protocol_state: Option<std::sync::Arc<dyn PeerStateProvider>>,
    announce_callback: Option<AnnounceCallback>,
}

impl Peer {
    pub fn builder() -> PeerBuilder {
        PeerBuilder::default()
    }

    pub fn id(&self) -> NodeId {
        *self.secret.public()
    }

    pub fn secret(&self) -> &SecretKey {
        &self.secret
    }

    pub fn blobs(&self) -> &BlobsStore {
        &self.blob_store
    }

    pub fn blobs_store_path(&self) -> &PathBuf {
        &self.blobs_store_path
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub async fn spawn(&self, mut shutdown_rx: WatchReceiver<()>) -> anyhow::Result<()> {
        // clone the blob store inner for the router
        let inner_blobs = self.blob_store.inner.clone();

        // Build the router against the endpoint -> to our blobs service
        //  NOTE (amiller68): if you want to extend our iroh capabilities
        //   with more protocols and handlers, you'd do so here
        let mut router_builder =
            Router::builder(self.endpoint.clone()).accept(iroh_blobs::ALPN, inner_blobs);

        // If we have protocol state, register the JAX protocol
        if let Some(state) = &self.protocol_state {
            let mut jax_protocol = JaxProtocol::new(state.clone());

            // Add announce callback if provided
            if let Some(callback) = &self.announce_callback {
                jax_protocol = jax_protocol.with_announce_callback(callback.clone());
            }

            router_builder = router_builder.accept(JAX_ALPN, jax_protocol);
            tracing::info!("JAX protocol registered");
        }

        let router = router_builder.spawn();

        // Wait for shutdown signal
        let _ = shutdown_rx.changed().await;

        router.shutdown().await?;
        Ok(())
    }
}
