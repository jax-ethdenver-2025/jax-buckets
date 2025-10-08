use clap::Args;

use crate::state::AppState;

#[derive(Args, Debug, Clone)]
pub struct Init;

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("init failed: {0}")]
    StateFailed(#[from] crate::state::StateError),
}

#[async_trait::async_trait]
impl crate::op::Op for Init {
    type Error = InitError;
    type Output = String;

    async fn execute(&self, _ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        let state = AppState::init()?;

        let output = format!(
            "Initialized jax directory at: {}\n\
             - Database: {}\n\
             - Key: {}\n\
             - Blobs: {}\n\
             - Config: {}\n\
             - Listen address: {}",
            state.jax_dir.display(),
            state.db_path.display(),
            state.key_path.display(),
            state.blobs_path.display(),
            state.config_path.display(),
            state.config.listen_addr
        );

        Ok(output)
    }
}
