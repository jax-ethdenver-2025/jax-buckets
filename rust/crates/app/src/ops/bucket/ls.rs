use service::http_server::api::client::ApiError;
use service::http_server::api::v0::bucket::ls::{LsRequest, LsResponse};

#[derive(Debug, thiserror::Error)]
pub enum BucketLsError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("Ls operation failed: {0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::op::Op for LsRequest {
    type Error = BucketLsError;
    type Output = String;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        // Call API
        let mut client = ctx.client.clone();
        let response: LsResponse = client.call(self.clone()).await?;

        if response.items.is_empty() {
            Ok("No items found".to_string())
        } else {
            let output = response
                .items
                .iter()
                .map(|item| {
                    let type_str = if item.is_dir { "dir" } else { "file" };
                    format!("{} ({}) [{}]", item.path, type_str, item.link.hash())
                })
                .collect::<Vec<_>>()
                .join("\n");
            Ok(output)
        }
    }
}
