use std::fmt;

use async_trait::async_trait;
use iroh::NodeId;
use iroh_blobs::Hash;

use jax::config::{Config, ConfigError};

use super::api_client::{api_requests, ApiClient, ApiError};
use crate::cli::args::Op;

#[derive(Debug, clap::Args, Clone)]
pub struct Query {
    /// The hash to query locations for
    #[clap(value_parser)]
    hash: Hash,
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("api error: {0}")]
    Api(#[from] ApiError),
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
}

#[derive(Debug)]
pub struct QueryOutput {
    hash: Hash,
    nodes: Vec<(NodeId, f64)>,
}

impl fmt::Display for QueryOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Locations for blob {}", self.hash)?;
        if self.nodes.is_empty() {
            writeln!(f, "  No known locations")?;
        } else {
            for (node, trust) in &self.nodes {
                writeln!(f, "  {} (trust: {:.3})", node, trust)?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Op for Query {
    type Error = QueryError;
    type Output = QueryOutput;

    async fn execute(&self) -> Result<Self::Output, Self::Error> {
        let config = Config::from_env_or_disk()?;
        let client = ApiClient::new(config.remote_url().as_ref())?;

        let request = api_requests::Query { hash: self.hash };

        let response = client.call(request).await?;

        Ok(QueryOutput {
            hash: self.hash,
            nodes: response.nodes,
        })
    }
}
