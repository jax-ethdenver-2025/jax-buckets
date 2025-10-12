use clap::Args;

use service::{spawn_service, ServiceConfig};

use crate::state::AppState;

#[derive(Args, Debug, Clone)]
pub struct Service {
    /// Run the HTML UI in read-only mode (hides write operations)
    #[arg(long)]
    pub ui_read_only: bool,

    /// API hostname to use for HTML UI (default: http://localhost:<api_port>)
    #[arg(long)]
    pub api_hostname: Option<String>,
}

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

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        // Load state from config path (or default ~/.jax)
        let state = AppState::load(ctx.config_path.clone())?;

        // Load the secret key
        let secret_key = state.load_key()?;

        // Build node listen address from peer_port if configured
        let node_listen_addr = state.config.peer_port.map(|port| {
            format!("0.0.0.0:{}", port)
                .parse()
                .expect("Failed to parse peer listen address")
        });

        // Build service config with persistent paths
        let config = ServiceConfig {
            node_listen_addr,
            node_secret: Some(secret_key),
            node_blobs_store_path: Some(state.blobs_path),
            html_listen_addr: state.config.html_listen_addr.parse().ok(),
            api_listen_addr: state.config.api_listen_addr.parse().ok(),
            sqlite_path: Some(state.db_path),
            log_level: tracing::Level::DEBUG,
            ui_read_only: self.ui_read_only,
            api_hostname: self.api_hostname.clone(),
        };

        spawn_service(&config).await;
        Ok("service ended".to_string())
    }
}
