use clap::Args;

use service::{spawn_service, ServiceConfig};

use crate::state::AppState;

#[derive(Args, Debug, Clone)]
pub struct Service;

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("state error: {0}")]
    StateError(#[from] crate::state::StateError),

    #[error("service failed: {0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::op::Op for Service {
    type Error = ServiceError;
    type Output = String;

    async fn execute(&self, _ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        // Load state from ~/.jax
        let state = AppState::load()?;

        // Load the secret key
        let secret_key = state.load_key()?;

        // Build service config with persistent paths
        let config = ServiceConfig {
            node_listen_addr: None, // Use ephemeral port for now, can be configured later
            node_secret: Some(secret_key),
            node_blobs_store_path: Some(state.blobs_path),
            http_listen_addr: state.config.listen_addr.parse().ok(),
            http_hostname: None,
            sqlite_path: Some(state.db_path),
            log_level: tracing::Level::INFO,
        };

        spawn_service(&config).await;
        Ok("service ended".to_string())
    }
}
