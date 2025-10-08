use std::fmt;

use async_trait::async_trait;

use jax::config::{Config, ConfigError};

use crate::cli::args::Op;

use super::api_client::{api_requests, ApiClient, ApiError};

#[derive(Debug, clap::Args, Clone)]
pub struct List {}

#[derive(Debug, thiserror::Error)]
pub enum ListError {
    #[error("api error: {0}")]
    Api(#[from] ApiError),
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
}

#[derive(Debug)]
pub struct ListOutput {
    blobs: Vec<String>,
}

impl fmt::Display for ListOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Stored blobs:")?;
        if self.blobs.is_empty() {
            writeln!(f, "  No blobs stored")?;
        } else {
            for hash in &self.blobs {
                writeln!(f, "  {}", hash)?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Op for List {
    type Error = ListError;
    type Output = ListOutput;

    async fn execute(&self) -> Result<Self::Output, Self::Error> {
        let config = Config::from_env_or_disk()?;
        let client = ApiClient::new(config.remote_url().as_ref())?;

        let request = api_requests::List;
        let response = client.call(request).await?;

        Ok(ListOutput {
            blobs: response.blobs.into_iter().map(|b| b.hash).collect(),
        })
    }
}
