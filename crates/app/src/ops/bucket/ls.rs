use clap::Args;
use service::http_server::api::client::ApiError;
use service::http_server::api::v0::bucket::ls::{LsRequest, LsResponse};
use uuid::Uuid;

#[derive(Args, Debug, Clone)]
pub struct Ls {
    /// Bucket ID (or use --name)
    #[arg(long, group = "bucket_identifier")]
    pub bucket_id: Option<Uuid>,

    /// Bucket name (or use --bucket-id)
    #[arg(long, group = "bucket_identifier")]
    pub name: Option<String>,

    /// Path in bucket to list (defaults to root)
    #[arg(long)]
    pub path: Option<String>,

    /// List recursively
    #[arg(long)]
    pub deep: Option<bool>,
}

#[derive(Debug, thiserror::Error)]
pub enum BucketLsError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("Either --bucket-id or --name must be provided")]
    NoBucketIdentifier,
}

#[async_trait::async_trait]
impl crate::op::Op for Ls {
    type Error = BucketLsError;
    type Output = String;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();

        // Resolve bucket name to UUID if needed
        let bucket_id = if let Some(id) = self.bucket_id {
            id
        } else if let Some(ref name) = self.name {
            client.resolve_bucket_name(name).await?
        } else {
            return Err(BucketLsError::NoBucketIdentifier);
        };

        // Create API request
        let request = LsRequest {
            bucket_id,
            path: self.path.clone(),
            deep: self.deep,
        };

        // Call API
        let response: LsResponse = client.call(request).await?;

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
