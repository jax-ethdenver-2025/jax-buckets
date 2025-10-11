use service::http_server::api::client::ApiError;
use service::http_server::api::v0::bucket::mount::{MountRequest, MountResponse};

#[derive(Debug, thiserror::Error)]
pub enum BucketMountError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("Mount operation failed: {0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::op::Op for MountRequest {
    type Error = BucketMountError;
    type Output = String;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        // Call API
        let mut client = ctx.client.clone();
        let response: MountResponse = client.call(self.clone()).await?;

        Ok(format!(
            "Mounted bucket: {} (id: {})\nBucket link: {}\nRoot link: {}",
            response.bucket_name,
            response.bucket_id,
            response.bucket_link.hash(),
            response.bucket_link.hash()
        ))
    }
}
