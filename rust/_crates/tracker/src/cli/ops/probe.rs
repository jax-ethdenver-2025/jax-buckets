use std::{fmt, str::FromStr};

use async_trait::async_trait;
use iroh::NodeId;
use iroh_blobs::Hash;

use jax::config::{Config, ConfigError};

use super::api_client::{api_requests, ApiClient, ApiError};
use crate::cli::args::Op;

#[derive(Debug, clap::Args, Clone)]
pub struct Probe {
    /// The hash to probe
    #[clap(long)]
    hash: String,
    /// The node to probe
    #[clap(short, long)]
    node: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error("api error: {0}")]
    Api(#[from] ApiError),
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
}

#[derive(Debug)]
pub struct ProbeOutput {
    hash: Hash,
    node: NodeId,
    stats: api_requests::ProbeStats,
}

impl fmt::Display for ProbeOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Probe results for {} from node {}:",
            self.hash, self.node
        )?;
        writeln!(f, "  Elapsed: {:?}", self.stats.elapsed)?;
        writeln!(f, "  Bytes read: {}", self.stats.bytes_read)?;
        writeln!(f, "  Bytes written: {}", self.stats.bytes_written)?;
        Ok(())
    }
}

#[async_trait]
impl Op for Probe {
    type Error = ProbeError;
    type Output = ProbeOutput;

    async fn execute(&self) -> Result<Self::Output, Self::Error> {
        let hash = Hash::from_str(&self.hash).expect("Invalid hash");
        let node = NodeId::from_str(&self.node).expect("Invalid node");
        let config = Config::from_env_or_disk()?;

        let client = ApiClient::new(config.remote_url().as_ref())?;

        let request = api_requests::Probe { hash, node };

        let response = client.call(request).await?;
        Ok(ProbeOutput {
            hash,
            node,
            stats: response.stats,
        })
    }
}
