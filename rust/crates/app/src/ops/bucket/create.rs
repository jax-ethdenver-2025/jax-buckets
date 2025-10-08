use service::http_server::api::client::ApiError;
use service::http_server::api::v0::bucket::create::{CreateRequest, CreateResponse};

#[derive(Debug, thiserror::Error)]
pub enum BucketCreateError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("Bucket operation failed: {0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::op::Op for CreateRequest {
    type Error = BucketCreateError;
    type Output = String;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        // Always use API client
        let mut client = ctx.client.clone();
        let response: CreateResponse = client.call(self.clone()).await?;

        Ok(format!(
            "Created bucket: {} (id: {}) at {}",
            response.name, response.bucket_id, response.created_at
        ))
    }
}
