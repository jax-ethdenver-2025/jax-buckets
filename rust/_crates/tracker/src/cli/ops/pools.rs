use std::fmt;

use alloy::primitives::Address;
use async_trait::async_trait;
use iroh::NodeId;
use iroh_blobs::Hash;

use jax::config::{Config, ConfigError};

use super::api_client::{api_requests, ApiClient, ApiError};
use crate::cli::args::Op;

#[derive(Debug, clap::Args, Clone)]
pub struct Pools {}

#[derive(Debug, thiserror::Error)]
pub enum PoolsError {
    #[error("api error: {0}")]
    Api(#[from] ApiError),
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
}

#[derive(Debug)]
pub struct PoolsOutput {
    pools: Vec<(Address, Hash, Vec<(NodeId, f64)>)>,
}

impl fmt::Display for PoolsOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Known pools:")?;
        if self.pools.is_empty() {
            writeln!(f, "  No pools found")?;
        } else {
            for (address, hash, peers) in &self.pools {
                writeln!(f, "  Pool {} for blob {}:", address, hash)?;
                for (node, trust) in peers {
                    writeln!(f, "    {} (trust: {:.3})", node, trust)?;
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Op for Pools {
    type Error = PoolsError;
    type Output = PoolsOutput;

    async fn execute(&self) -> Result<Self::Output, Self::Error> {
        let config = Config::from_env_or_disk()?;
        let client = ApiClient::new(config.remote_url().as_ref())?;

        let request = api_requests::Pools {};
        let response = client.call(request).await?;

        Ok(PoolsOutput {
            pools: response.pools,
        })
    }
}
