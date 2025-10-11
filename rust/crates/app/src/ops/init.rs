use clap::Args;

use crate::state::{AppConfig, AppState};

#[derive(Args, Debug, Clone)]
pub struct Init {
    /// HTML server listen address (default: 0.0.0.0:8080)
    #[arg(long, default_value = "0.0.0.0:8080")]
    pub html_addr: String,

    /// API server listen address (default: 0.0.0.0:3000)
    #[arg(long, default_value = "0.0.0.0:3000")]
    pub api_addr: String,

    /// Peer (P2P) node listen port (optional, defaults to ephemeral port if not specified)
    #[arg(long)]
    pub peer_port: Option<u16>,
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("init failed: {0}")]
    StateFailed(#[from] crate::state::StateError),
}

#[async_trait::async_trait]
impl crate::op::Op for Init {
    type Error = InitError;
    type Output = String;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        let config = AppConfig {
            html_listen_addr: self.html_addr.clone(),
            api_listen_addr: self.api_addr.clone(),
            peer_port: self.peer_port,
        };

        let state = AppState::init(ctx.config_path.clone(), Some(config))?;

        let peer_port_str = match state.config.peer_port {
            Some(port) => format!("{}", port),
            None => "ephemeral (auto-assigned)".to_string(),
        };

        let output = format!(
            "Initialized jax directory at: {}\n\
             - Database: {}\n\
             - Key: {}\n\
             - Blobs: {}\n\
             - Config: {}\n\
             - HTML listen address: {}\n\
             - API listen address: {}\n\
             - Peer port: {}",
            state.jax_dir.display(),
            state.db_path.display(),
            state.key_path.display(),
            state.blobs_path.display(),
            state.config_path.display(),
            state.config.html_listen_addr,
            state.config.api_listen_addr,
            peer_port_str
        );

        Ok(output)
    }
}
