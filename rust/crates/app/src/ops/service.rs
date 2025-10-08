use clap::Args;

use service::{spawn_service, ServiceConfig};

#[derive(Args, Debug, Clone)]
pub struct Service;

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    // TODO (amiller68): be much better at this
    #[error("service failed: {0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::op::Op for Service {
    type Error = ServiceError;
    type Output = String;

    async fn execute(&self, _ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        spawn_service(&ServiceConfig::default()).await;
        Ok("service ended".to_string())
    }
}
