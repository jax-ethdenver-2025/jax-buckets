use clap::Args;

use common::build_info;

#[derive(Args, Debug, Clone)]
pub struct Version;

#[derive(Debug, thiserror::Error)]
pub enum VersionError {
    #[error("Version operation failed: {0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::op::Op for Version {
    type Error = VersionError;
    type Output = String;

    async fn execute(&self, _ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        Ok(build_info!().to_string())
    }
}
