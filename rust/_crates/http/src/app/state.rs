use std::convert::TryFrom;
use std::sync::Arc;

use axum::extract::FromRef;
use url::Url;

use leaky_common::prelude::*;

use super::config::Config;
use crate::database::{models::RootCid, Database};
use parking_lot::RwLock;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct AppState {
    get_content_forwarding_url: Url,
    sqlite_database: Database,
    mount: Arc<RwLock<Mount>>,
    request_semaphore: Arc<Semaphore>,
}

impl AppState {
    pub fn get_content_forwarding_url(&self) -> &Url {
        &self.get_content_forwarding_url
    }

    pub fn sqlite_database(&self) -> &Database {
        &self.sqlite_database
    }

    pub fn mount_for_reading(&self) -> Mount {
        self.mount.read().clone()
    }

    pub fn mount(&self) -> Arc<RwLock<Mount>> {
        self.mount.clone()
    }

    pub async fn from_config(config: &Config) -> Result<Self, AppStateSetupError> {
        let sqlite_database = Database::connect(config.sqlite_database_url()).await?;
        let ipfs_rpc = IpfsRpc::try_from(config.ipfs_rpc_url().clone())?;
        let mut conn = sqlite_database.acquire().await?;
        let maybe_root_cid = RootCid::pull(&mut conn).await?;
        let mount = match maybe_root_cid {
            Some(rc) => Mount::pull(rc.cid(), &ipfs_rpc).await?,
            None => {
                let mount = Mount::init(&ipfs_rpc).await?;
                let previous_cid = mount.previous_cid();
                let cid = mount.cid();
                RootCid::push(cid, &previous_cid, &mut conn).await?;
                mount
            }
        };

        Ok(Self {
            get_content_forwarding_url: config.get_content_forwarding_url().clone(),
            sqlite_database,
            mount: Arc::new(RwLock::new(mount)),
            request_semaphore: Arc::new(Semaphore::new(100)),
        })
    }

    pub fn request_semaphore(&self) -> Arc<Semaphore> {
        self.request_semaphore.clone()
    }

    pub fn mount_guard_mut(&self) -> MountGuardMut {
        let guard = unsafe {
            std::mem::transmute::<
                parking_lot::RwLockWriteGuard<'_, Mount>,
                parking_lot::RwLockWriteGuard<'static, Mount>,
            >(self.mount.write())
        };
        MountGuardMut { _lock: guard }
    }
}

impl FromRef<AppState> for Database {
    fn from_ref(app_state: &AppState) -> Self {
        app_state.sqlite_database.clone()
    }
}

// impl FromRef<AppState> for IpfsRpc {
//     fn from_ref(app_state: &AppState) -> Self {
//         app_state.ipfs_rpc.clone()
//     }
// }

impl FromRef<AppState> for Arc<RwLock<Mount>> {
    fn from_ref(app_state: &AppState) -> Self {
        app_state.mount.clone()
    }
}

pub struct MountGuardMut {
    _lock: parking_lot::RwLockWriteGuard<'static, Mount>,
}

impl MountGuardMut {
    pub async fn update(&mut self, cid: Cid) -> Result<(), MountError> {
        self._lock.update(cid).await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppStateSetupError {
    #[error("failed to setup the database: {0}")]
    DatabaseSetup(#[from] crate::database::DatabaseSetupError),
    #[error("sqlx: {0}")]
    Database(#[from] sqlx::Error),
    #[error("failed to setup the IPFS RPC client: {0}")]
    IpfsRpcError(#[from] leaky_common::error::IpfsRpcError),
    #[error("root CID error: {0}")]
    RootCid(#[from] crate::database::models::RootCidError),
    #[error("mount error: {0}")]
    Mount(#[from] MountError),
    #[error("Unsupported image format")]
    UnsupportedImageFormat,
}
