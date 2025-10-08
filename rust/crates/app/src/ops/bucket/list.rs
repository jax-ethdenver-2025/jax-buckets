use service::http_server::api::client::ApiError;
use service::http_server::api::v0::bucket::list::{ListRequest, ListResponse};

#[derive(Debug, thiserror::Error)]
pub enum BucketListError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("Bucket list operation failed: {0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::op::Op for ListRequest {
    type Error = BucketListError;
    type Output = String;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        // Always use API client
        let mut client = ctx.client.clone();
        let response: ListResponse = client.call(self.clone()).await?;

        if response.buckets.is_empty() {
            Ok("No buckets found".to_string())
        } else {
            let output = response
                .buckets
                .iter()
                .map(|b| format!("{} (id: {} | link: {})", b.name, b.bucket_id, b.link.hash()))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(output)
        }
    }
}
