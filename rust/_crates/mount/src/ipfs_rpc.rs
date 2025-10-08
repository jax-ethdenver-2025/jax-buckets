use std::convert::TryFrom;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;

use futures_util::TryStreamExt;
use http::uri::Scheme;
use ipfs_api_backend_hyper::request::{Add as AddRequest, BlockPut as BlockPutRequest};
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use url::Url;

use crate::types::{
    block_from_data, ipld_from_block, ipld_to_block, Cid, Ipld, DEFAULT_CID_VERSION,
    DEFAULT_HASH_CODE_STRING, DEFAULT_IPLD_CODEC_STRING,
};

#[derive(Clone)]
pub struct IpfsRpc {
    client: IpfsClient,
}

impl Default for IpfsRpc {
    fn default() -> Self {
        let url: Url = "http://localhost:5001".try_into().unwrap();
        Self::try_from(url).unwrap()
    }
}

impl TryFrom<Url> for IpfsRpc {
    type Error = IpfsRpcError;
    fn try_from(url: Url) -> Result<Self, IpfsRpcError> {
        let scheme = Scheme::try_from(url.scheme())?;
        let host_str = url
            .host_str()
            .ok_or(IpfsRpcError::Url(url::ParseError::EmptyHost))?;
        let port = url.port().unwrap_or(443);
        let client = IpfsClient::from_host_and_port(scheme, host_str, port)?;
        Ok(Self { client })
    }
}

impl IpfsRpc {
    pub fn with_bearer_token(mut self, token: String) -> Self {
        self.client = self.client.with_bearer_token(token);
        self
    }

    pub fn with_path(mut self, path: &str) -> Self {
        let path = PathBuf::from(path);
        self.client = self.client.with_path(path);
        self
    }

    pub async fn has_pinned(&self, cid: &Cid) -> Result<bool, IpfsRpcError> {
        let cid = *cid;
        let client = self.client.clone();
        let response = tokio::task::spawn_blocking(move || {
            tokio::runtime::Handle::current()
                .block_on(async move { client.pin_ls(Some(&cid.to_string()), None).await })
        })
        .await
        .map_err(|e| IpfsRpcError::Default(anyhow::anyhow!("Join error: {}", e)))??;

        let keys = response.keys;
        Ok(keys.contains_key(&cid.to_string()))
    }

    pub async fn hash_data<R>(&self, data: R) -> Result<Cid, IpfsRpcError>
    where
        R: Read + Send + Sync + 'static + Unpin,
    {
        let options = AddRequest {
            hash: Some(DEFAULT_HASH_CODE_STRING),
            cid_version: Some(DEFAULT_CID_VERSION),
            only_hash: Some(true),
            ..Default::default()
        };
        let client = self.client.clone();
        let response = tokio::task::spawn_blocking(move || {
            tokio::runtime::Handle::current()
                .block_on(async move { client.add_with_options(data, options).await })
        })
        .await
        .map_err(|e| IpfsRpcError::Default(anyhow::anyhow!("Join error: {}", e)))??;

        let cid = Cid::from_str(&response.hash)?;
        Ok(cid)
    }

    pub async fn add_data<R>(&self, data: R) -> Result<Cid, IpfsRpcError>
    where
        R: Read + Send + Sync + 'static + Unpin,
    {
        let options = AddRequest {
            hash: Some(DEFAULT_HASH_CODE_STRING),
            cid_version: Some(DEFAULT_CID_VERSION),
            ..Default::default()
        };

        let client = self.client.clone();

        let response = tokio::task::spawn_blocking(move || {
            tokio::runtime::Handle::current()
                .block_on(async move { client.add_with_options(data, options).await })
        })
        .await
        .map_err(|e| IpfsRpcError::Default(anyhow::anyhow!("Join error: {}", e)))??;

        let cid = Cid::from_str(&response.hash)?;

        Ok(cid)
    }

    pub async fn cat_data(&self, cid: &Cid) -> Result<Vec<u8>, IpfsRpcError> {
        let client = self.client.clone();
        let cid_string = cid.to_string();

        // Spawn a blocking task to perform the potentially blocking operation
        let result = tokio::task::spawn_blocking(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    client
                        .cat(&cid_string)
                        .map_ok(|chunk| chunk.to_vec())
                        .try_concat()
                        .await
                })
        })
        .await
        .map_err(|e| IpfsRpcError::Default(anyhow::anyhow!("Join error: {}", e)))??;

        Ok(result)
    }

    // NOTE: had to wrap the client call in a spawn_blocking because the client doesn't implement Send
    pub async fn put_ipld<T: Into<Ipld>>(&self, ipld: T) -> Result<Cid, IpfsRpcError> {
        let ipld = ipld.into();
        let block = ipld_to_block(ipld);
        let cursor = std::io::Cursor::new(block.data().to_vec());
        let options = BlockPutRequest {
            mhtype: Some(DEFAULT_HASH_CODE_STRING),
            cid_codec: Some(DEFAULT_IPLD_CODEC_STRING),
            pin: Some(true),
            ..Default::default()
        };

        let client = self.client.clone();
        let result = tokio::task::spawn_blocking(move || {
            tokio::runtime::Handle::current()
                .block_on(async move { client.block_put_with_options(cursor, options).await })
        })
        .await
        .map_err(|e| IpfsRpcError::Default(anyhow::anyhow!("Join error: {}", e)))??;

        let cid = Cid::from_str(&result.key)?;

        Ok(cid)
    }

    pub async fn get_ipld(&self, cid: &Cid) -> Result<Ipld, IpfsRpcError> {
        let cid = *cid;
        let client = self.client.clone();
        tokio::task::spawn_blocking(move || {
            tokio::runtime::Handle::current().block_on(async move {
                let stream = client.block_get(&cid.to_string());
                let block_data = stream.map_ok(|chunk| chunk.to_vec()).try_concat().await?;
                let block = block_from_data(cid, block_data)?;
                let ipld = ipld_from_block(block)?;
                Ok(ipld)
            })
        })
        .await
        .map_err(|e| IpfsRpcError::Default(anyhow::anyhow!("Join error: {}", e)))?
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IpfsRpcError {
    #[error("default error: {0}")]
    Default(#[from] anyhow::Error),
    #[error("url parse error")]
    Url(#[from] url::ParseError),
    #[error("http error")]
    Http(#[from] http::Error),
    #[error("Failed to parse scheme")]
    Scheme(#[from] http::uri::InvalidUri),
    #[error("Failed to build client: {0}")]
    Client(#[from] ipfs_api_backend_hyper::Error),
    #[error("cid error")]
    Cid(#[from] crate::types::CidError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use libipld::IpldCodec;
    use std::collections::BTreeMap;

    /// Generate a random 1 KB reader
    fn random_reader() -> impl Read {
        use rand::Rng;
        use std::io::Cursor;
        let mut rng = rand::thread_rng();
        let data: Vec<u8> = (0..1024).map(|_| rng.gen()).collect();
        Cursor::new(data)
    }

    #[tokio::test]
    async fn test_add_data_cat_data() {
        let ipfs = IpfsRpc::default();
        let data = std::io::Cursor::new(b"hello world");
        let cid = ipfs.add_data(data).await.unwrap();
        let cat_data = ipfs.cat_data(&cid).await.unwrap();
        assert_eq!(cat_data.len(), 11);
        assert_eq!(cat_data, b"hello world");
    }

    #[tokio::test]
    async fn test_add_data_blake3_256_raw() {
        let ipfs = IpfsRpc::default();
        let data = random_reader();
        let cid = ipfs.add_data(data).await.unwrap();
        assert_eq!(cid.version(), libipld::cid::Version::V1);
        assert_eq!(IpldCodec::try_from(cid.codec()).unwrap(), IpldCodec::Raw);
        assert_eq!(cid.hash().code(), 0x1e);
    }

    #[tokio::test]
    async fn test_put_block_blake3_256_dag_cbor() {
        let ipfs = IpfsRpc::default();
        let mut map = BTreeMap::new();
        map.insert("hello".to_string(), Ipld::String("world".to_string()));
        let cid = ipfs.put_ipld(Ipld::Map(map)).await.unwrap();
        assert_eq!(cid.version(), libipld::cid::Version::V1);
        assert_eq!(
            IpldCodec::try_from(cid.codec()).unwrap(),
            IpldCodec::DagCbor
        );
        assert_eq!(cid.hash().code(), 0x1e);
    }
}
