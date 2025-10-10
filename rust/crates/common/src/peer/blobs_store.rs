use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use anyhow::anyhow;
use bytes::Bytes;
use futures::Stream;
use iroh::Endpoint;
use iroh_blobs::rpc::client::blobs::{BlobStatus, Reader};
use iroh_blobs::util::SetTagOption;
use iroh_blobs::{net_protocol::Blobs, store::fs::Store, ticket::BlobTicket, Hash};

// TODO (amiller68): maybe at some point it would make sense
//  to implement some sort of `BlockStore` trait over BlobStore
/// Client over a local iroh-blob store.
///  Exposes an iroh-blobs peer over the endpoint.
///  Router must handle the iroh-blobs APLN
/// Also acts as our main BlockStore implemenetation
///  for bucket, node, and data storage and retrieval
#[derive(Clone, Debug)]
pub struct BlobsStore {
    pub inner: Arc<Blobs<Store>>,
}

impl Deref for BlobsStore {
    type Target = Arc<Blobs<Store>>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BlobsStoreError {
    #[error("blobs store error: {0}")]
    Default(#[from] anyhow::Error),
    #[error("blob store i/o error: {0}")]
    Io(#[from] std::io::Error),
}

impl BlobsStore {
    /// Load a blob store from the given path, using the given endpoint.
    ///  Endpoint exposes a network interface for blob operations
    ///  with peers.
    ///
    /// # Arguments
    /// * `path` - Path to the blob store on disk
    /// * `endpoint` - Endpoint to use for network operations
    ///     Exposes a peer for the private key used to initiate
    ///     the endpoint.
    #[allow(clippy::doc_overindented_list_items)]
    pub async fn load(path: &Path, endpoint: Endpoint) -> Result<Self, BlobsStoreError> {
        let store = Store::load(path).await?;
        let blobs = Blobs::builder(store).build(&endpoint);
        Ok(Self {
            inner: Arc::new(blobs),
        })
    }

    /// Get a blob as bytes
    pub async fn get(&self, hash: &Hash) -> Result<Bytes, BlobsStoreError> {
        let bytes = self.client().read_to_bytes(*hash).await?;
        Ok(bytes)
    }

    /// Get a blob from the store as a reader
    pub async fn get_reader(&self, hash: Hash) -> Result<Reader, BlobsStoreError> {
        let reader = self.client().read(hash).await?;
        Ok(reader)
    }

    /// Store a stream of bytes as a blob
    pub async fn put_stream(
        &self,
        stream: impl Stream<Item = std::io::Result<Bytes>> + Send + Unpin + 'static,
    ) -> Result<Hash, BlobsStoreError> {
        let outcome = self
            .client()
            .add_stream(stream, SetTagOption::Auto)
            .await
            .map_err(|e| anyhow!(e))?
            .finish()
            .await
            .map_err(|e| anyhow!(e))?;
        Ok(outcome.hash)
    }

    /// Store a vec of bytes as a blob
    pub async fn put(&self, data: Vec<u8>) -> Result<Hash, BlobsStoreError> {
        let hash = self.client().add_bytes(data).await?.hash;
        Ok(hash)
    }

    /// Get the stat of a blob
    pub async fn stat(&self, hash: &Hash) -> Result<bool, BlobsStoreError> {
        let stat = self.client().status(*hash).await?;
        Ok(matches!(stat, BlobStatus::Complete { .. }))
    }

    /// Pull a blob from the network using its ticket
    ///  Specify a ticker by concatenating the node address
    ///  of a peer that has the blob, with the blob hash and format
    pub async fn pull(&self, ticket: &BlobTicket) -> Result<(), BlobsStoreError> {
        self.client()
            .download(ticket.hash(), ticket.node_addr().clone())
            .await?
            .finish()
            .await?;
        Ok(())
    }

    /// Create a simple blob containing a sequence of hashes
    /// Each hash is 32 bytes, stored consecutively
    /// Returns the hash of the blob containing all the hashes
    pub async fn create_hash_list<I>(&self, hashes: I) -> Result<Hash, BlobsStoreError>
    where
        I: IntoIterator<Item = Hash>,
    {
        // Serialize hashes as raw bytes (32 bytes each, concatenated)
        let mut data = Vec::new();
        for hash in hashes {
            data.extend_from_slice(hash.as_bytes());
        }

        // Store as a single blob
        let hash = self.put(data).await?;
        Ok(hash)
    }

    /// Read all hashes from a hash list blob
    /// Returns a Vec of all hashes in the list
    pub async fn read_hash_list(&self, list_hash: Hash) -> Result<Vec<Hash>, BlobsStoreError> {
        let mut hashes = Vec::new();

        // Read the blob
        let data = self.get(&list_hash).await?;

        // Parse hashes (32 bytes each)
        if data.len() % 32 != 0 {
            return Err(anyhow!("Invalid hash list: length is not a multiple of 32").into());
        }

        for chunk in data.chunks_exact(32) {
            let mut hash_bytes = [0u8; 32];
            hash_bytes.copy_from_slice(chunk);
            hashes.push(Hash::from_bytes(hash_bytes));
        }

        Ok(hashes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures::stream;
    use tempfile::TempDir;

    async fn setup_test_store() -> (BlobsStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let blob_path = temp_dir.path().join("blobs");

        let secret_key = iroh::SecretKey::generate(rand_core::OsRng);
        let endpoint = iroh::Endpoint::builder()
            .secret_key(secret_key)
            .bind()
            .await
            .unwrap();

        let store = BlobsStore::load(&blob_path, endpoint).await.unwrap();
        (store, temp_dir)
    }

    #[tokio::test]
    async fn test_put_and_get() {
        let (store, _temp) = setup_test_store().await;

        // Test data
        let data = b"Hello, BlobsStore!";

        // Put data
        let hash = store.put(data.to_vec()).await.unwrap();
        assert!(!hash.as_bytes().is_empty());

        // Get data back
        let retrieved = store.get(&hash).await.unwrap();
        assert_eq!(retrieved.as_ref(), data);
    }

    #[tokio::test]
    async fn test_put_stream() {
        let (store, _temp) = setup_test_store().await;

        // Create a stream of data
        let data = b"Streaming data test";
        let stream =
            stream::once(async move { Ok::<_, std::io::Error>(Bytes::from(data.to_vec())) });

        // Put stream
        let hash = store.put_stream(Box::pin(stream)).await.unwrap();

        // Verify we can get it back
        let retrieved = store.get(&hash).await.unwrap();
        assert_eq!(retrieved.as_ref(), data);
    }

    #[tokio::test]
    async fn test_stat() {
        let (store, _temp) = setup_test_store().await;

        let data = b"Test data for stat";
        let hash = store.put(data.to_vec()).await.unwrap();

        // Should exist
        assert!(store.stat(&hash).await.unwrap());

        // Non-existent hash should not exist
        let fake_hash = iroh_blobs::Hash::from_bytes([0u8; 32]);
        assert!(!store.stat(&fake_hash).await.unwrap());
    }

    #[tokio::test]
    async fn test_large_data() {
        let (store, _temp) = setup_test_store().await;

        // Create large data (1MB)
        let data = vec![42u8; 1024 * 1024];

        // Put and get large data
        let hash = store.put(data.clone()).await.unwrap();
        let retrieved = store.get(&hash).await.unwrap();

        assert_eq!(retrieved.len(), data.len());
        assert_eq!(retrieved.as_ref(), data.as_slice());
    }

    #[tokio::test]
    async fn test_multiple_puts() {
        let (store, _temp) = setup_test_store().await;

        let data1 = b"First data";
        let data2 = b"Second data";
        let data3 = b"Third data";

        // Put multiple items
        let hash1 = store.put(data1.to_vec()).await.unwrap();
        let hash2 = store.put(data2.to_vec()).await.unwrap();
        let hash3 = store.put(data3.to_vec()).await.unwrap();

        // Verify all are different hashes
        assert_ne!(hash1, hash2);
        assert_ne!(hash2, hash3);
        assert_ne!(hash1, hash3);

        // Verify all can be retrieved
        assert_eq!(store.get(&hash1).await.unwrap().as_ref(), data1);
        assert_eq!(store.get(&hash2).await.unwrap().as_ref(), data2);
        assert_eq!(store.get(&hash3).await.unwrap().as_ref(), data3);
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let (store, _temp) = setup_test_store().await;

        // Try to get non-existent data
        let fake_hash = iroh_blobs::Hash::from_bytes([99u8; 32]);
        let result = store.get(&fake_hash).await;

        // Should return an error
        assert!(result.is_err());
    }
}
