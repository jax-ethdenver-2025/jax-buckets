use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;

use iroh::discovery::pkarr::dht::DhtDiscovery;
use iroh::{protocol::Router, Endpoint, NodeId};
use tokio::sync::watch::Receiver as WatchReceiver;

use crate::crypto::SecretKey;

mod blobs_store;

pub use blobs_store::{BlobsStore, BlobsStoreError};

#[derive(Debug, Clone, Default)]
pub struct NodeBuilder {
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
}

// TODO (amiller68): proper errors
impl NodeBuilder {
    pub fn new() -> Self {
        NodeBuilder {
            socket_addr: None,
            secret_key: None,
            blobs_store_path: None,
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

    pub async fn build(self) -> Node {
        // set the socket port to unspecified if not set
        let socket_addr = self
            .socket_addr
            .unwrap_or_else(|| SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0));
        // generate a new secret key if not set
        let secret_key = self.secret_key.unwrap_or_else(SecretKey::generate);
        // and set the blobs store path to a temporary directory if not set
        let blobs_store_path = self.blobs_store_path.unwrap_or_else(|| {
            // Create a temporary directory for the blobs store
            let temp_dir = tempfile::tempdir().expect("failed to create temporary directory");
            temp_dir.path().to_path_buf()
        });

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
            .discovery(Box::new(mainline_discovery))
            .bind_addr_v4(addr)
            .bind()
            .await
            .expect("failed to bind ephemeral endpoint");

        // Create the blob store
        let blob_store = BlobsStore::load(&blobs_store_path, endpoint.clone())
            .await
            .expect("failed to load blob store");

        Node {
            blob_store,
            secret: secret_key,
            endpoint,
            blobs_store_path,
        }
    }
}

// TODO (amiller68): this can prolly be simpler /
//  idk if we need all of this, but it'll work for now
#[derive(Debug, Clone)]
pub struct Node {
    blob_store: BlobsStore,
    secret: SecretKey,
    endpoint: Endpoint,
    blobs_store_path: PathBuf,
}

impl Node {
    pub fn builder() -> NodeBuilder {
        NodeBuilder::default()
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

    pub async fn spawn(&self, mut shutdown_rx: WatchReceiver<()>) -> anyhow::Result<()> {
        // clone the blob store inner for the router
        let inner_blobs = self.blob_store.inner.clone();
        // Build the router against the endpoint -> to our blobs service
        //  NOTE (amiller68): if you want to extend our iroh capabilities
        //   with more protocols and handlers, you'd do so here
        let router = Router::builder(self.endpoint.clone())
            .accept(iroh_blobs::ALPN, inner_blobs)
            .spawn();

        // Wait for shutdown signal
        let _ = shutdown_rx.changed().await;

        router.shutdown().await?;
        Ok(())
    }
}
