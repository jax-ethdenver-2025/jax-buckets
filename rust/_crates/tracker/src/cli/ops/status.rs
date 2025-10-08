use alloy::primitives::Address;
use async_trait::async_trait;

use iroh::NodeId;
use jax::config::{Config, ConfigError};

use super::api_client::{
    requests::{Liveness, Readiness},
    ApiClient, ApiError,
};

use crate::cli::args::Op;

#[derive(Debug, clap::Args, Clone)]
pub struct Status {}

#[derive(Debug, thiserror::Error)]
pub enum StatusError {
    #[error("api error: {0}")]
    Api(#[from] ApiError),
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
}

#[derive(Debug, Clone)]
pub struct StatusOutput {
    node_id: NodeId,
    eth_address: Address,
}

impl std::fmt::Display for StatusOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Server Status:\n- Node ID: {}\n- ETH Address: {}",
            self.node_id, self.eth_address
        )
    }
}

#[async_trait]
impl Op for Status {
    type Error = StatusError;
    type Output = StatusOutput;

    async fn execute(&self) -> Result<Self::Output, Self::Error> {
        let config = Config::from_env_or_disk()?;
        let client = ApiClient::new(config.remote_url().as_ref())?;

        // Check readiness
        let readiness = client.call(Readiness).await?;

        Ok(StatusOutput {
            node_id: readiness.node_id,
            eth_address: readiness.eth_address,
        })
    }
}
