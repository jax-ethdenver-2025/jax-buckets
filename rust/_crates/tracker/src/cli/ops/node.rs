use async_trait::async_trait;

use jax::config::{Config, ConfigError};
use jax::node::Node as _Node;

use crate::cli::args::Op;

#[derive(Debug, clap::Args, Clone)]
pub struct Node;

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("default error: {0}")]
    Default(#[from] anyhow::Error),
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
}

#[async_trait]
impl Op for Node {
    type Error = NodeError;
    type Output = ();

    async fn execute(&self) -> Result<Self::Output, Self::Error> {
        let config = Config::from_env_or_disk()?;
        _Node::spawn(&config).await;
        Ok(())
    }
}
