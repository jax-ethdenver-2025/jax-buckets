use crate::{AppState, Op};
use async_trait::async_trait;
use leaky_common::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;
use thumbs_up::prelude::{EcKey, PrivateKey, PublicKey};
use url::Url;

#[derive(Debug, clap::Args, Clone)]
pub struct Init {
    // NOTE: not used in exexute, but when initializing the app state
    #[clap(short, long)]
    pub remote: Url,
    #[clap(short, long)]
    pub key_path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("default error: {0}")]
    Default(#[from] anyhow::Error),
    #[error("app state error: {0}")]
    AppState(#[from] crate::state::AppStateSetupError),
    #[error("ipfs error: {0}")]
    Ipfs(#[from] leaky_common::error::IpfsRpcError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("api error: {0}")]
    Api(#[from] leaky_common::error::ApiError),
    #[error("mount error: {0}")]
    Mount(#[from] leaky_common::error::MountError),
    #[error("remote already initialized")]
    RemoteAlreadyInitialized,
    #[error("thumbs up error: {0}")]
    ThumbsUp(#[from] thumbs_up::prelude::KeyError),
}

#[async_trait]
impl Op for Init {
    type Error = InitError;
    type Output = (Cid, PathBuf);

    async fn execute(&self, state: &AppState) -> Result<Self::Output, Self::Error> {
        let key = EcKey::generate()?;
        let private_key_pem = key.export()?;
        let public_key_pem = key.public_key()?.export()?;
        // Check if the path is directory
        let path = std::path::Path::new(&self.key_path);
        let private_key_path = path.join("leaky.prv");
        let public_key_path = path.join("leaky.pem");
        if path.is_dir() && !private_key_path.exists() && !public_key_path.exists() {
            std::fs::write(private_key_path, private_key_pem)?;
            std::fs::write(public_key_path.clone(), public_key_pem)?;
        }
        let mut client = state.client()?;
        let ipfs_rpc = Arc::new(client.ipfs_rpc()?);
        let cid = client.call(PullRoot).await?;

        let mount = Mount::pull(cid.cid(), &ipfs_rpc.clone()).await?;

        state.save(&mount, None, Some(*mount.cid()))?;

        Ok((*mount.cid(), public_key_path))
    }
}
