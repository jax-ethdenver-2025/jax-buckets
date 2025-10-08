use async_trait::async_trait;
use iroh::NodeId;
use iroh_blobs::Hash;
use std::path::PathBuf;

use jax::config::{Config, ConfigError};

use super::api_client::{api_requests, ApiClient, ApiError};
use crate::cli::args::Op as AppOp;

#[derive(Debug, clap::Args, Clone)]
pub struct Share {
    /// Path to the file to share
    #[arg(short, long)]
    pub path: PathBuf,

    /// Create a pool for this content
    #[arg(short, long)]
    pub create_pool: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ShareError {
    #[error("api error: {0}")]
    Api(#[from] ApiError),
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[async_trait]
impl AppOp for Share {
    type Error = ShareError;
    type Output = String;

    async fn execute(&self) -> Result<Self::Output, Self::Error> {
        let config = Config::from_env_or_disk()?;
        let client = ApiClient::new(config.remote_url().as_ref())?;

        // Get absolute path for the file
        let abs_path = std::path::absolute(&self.path)?;

        // Create share request
        let request = api_requests::Share {
            path: abs_path.to_string_lossy().to_string(),
        };

        // Call the API endpoint to share the file
        let response = client.call(request).await?;

        let mut output = format!(
            "{}\nShare ticket: {}\nHash: {}",
            response.message, response.ticket, response.hash
        );

        // If create_pool flag is set, create a pool for this content
        if self.create_pool {
            // Create pool request
            let create_pool_request = api_requests::CreatePool {
                hash: response.hash.clone(),
            };

            // Call the API endpoint to create a pool
            let pool_response = client.call(create_pool_request).await?;

            output.push_str(&format!("\n\nPool created: {}", pool_response.success));
        }

        Ok(output)
    }
}
