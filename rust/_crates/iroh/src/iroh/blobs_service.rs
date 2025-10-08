use anyhow::{anyhow, Result};
use bytes::Bytes;
use futures::Stream;
use iroh::Endpoint;
use iroh_blobs::rpc::client::blobs::{BlobStatus, Reader};
use iroh_blobs::util::SetTagOption;
use iroh_blobs::{net_protocol::Blobs, store::fs::Store, ticket::BlobTicket, Hash};
use std::path::Path;
use std::sync::Arc;

/// Service that handles blob operations
#[derive(Clone, Debug)]
pub struct BlobsService {
    blobs: Arc<Blobs<Store>>,
}

impl BlobsService {
    /// Create a new blob service
    pub async fn load(blobs_path: &Path, endpoint: Endpoint) -> Result<Self> {
        let store = Store::load(blobs_path).await?;
        let blobs = Blobs::builder(store).build(&endpoint);
        Ok(Self {
            blobs: Arc::new(blobs),
        })
    }

    /// Store a stream as a blob
    pub async fn store_stream(
        &self,
        stream: impl Stream<Item = std::io::Result<Bytes>> + Send + Unpin + 'static,
    ) -> Result<Hash> {
        let outcome = self
            .blobs
            .client()
            .add_stream(stream, SetTagOption::Auto)
            .await
            .map_err(|e| anyhow!(e))?
            .finish()
            .await
            .map_err(|e| anyhow!(e))?;
        Ok(outcome.hash)
    }

    /// Store a blob with the given format
    pub async fn store_blob(&self, data: Vec<u8>) -> Result<Hash> {
        let hash = self.blobs.client().add_bytes(data).await?.hash;
        Ok(hash)
    }

    /// Get the stat of a blob
    pub async fn get_blob_stat(&self, hash: Hash) -> Result<bool> {
        let stat = self.blobs.client().status(hash).await?;
        Ok(matches!(stat, BlobStatus::Complete { .. }))
    }

    // TODO: get ticket

    /// Get the underlying Blobs instance
    pub fn get_inner_blobs(&self) -> &Arc<Blobs<Store>> {
        &self.blobs
    }

    /// Get a blob using a ticket
    pub async fn download_blob(&self, ticket: &BlobTicket) -> Result<()> {
        self.blobs
            .client()
            .download(ticket.hash(), ticket.node_addr().clone())
            .await?
            .finish()
            .await?;
        Ok(())
    }

    /// Read a blob from the given reader
    pub async fn read_blob(&self, hash: Hash) -> Result<Reader> {
        self.blobs.client().read(hash).await
    }
}
