use base64::Engine;
use clap::Args;
use service::http_server::api::client::ApiError;
use service::http_server::api::v0::bucket::cat::{CatRequest, CatResponse};
use uuid::Uuid;

#[derive(Args, Debug, Clone)]
pub struct Cat {
    /// Bucket ID (or use --name)
    #[arg(long, group = "bucket_identifier")]
    pub bucket_id: Option<Uuid>,

    /// Bucket name (or use --bucket-id)
    #[arg(long, group = "bucket_identifier")]
    pub name: Option<String>,

    /// Path in bucket to read
    #[arg(long)]
    pub path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum BucketCatError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("Either --bucket-id or --name must be provided")]
    NoBucketIdentifier,
}

#[async_trait::async_trait]
impl crate::op::Op for Cat {
    type Error = BucketCatError;
    type Output = String;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();

        // Resolve bucket name to UUID if needed
        let bucket_id = if let Some(id) = self.bucket_id {
            id
        } else if let Some(ref name) = self.name {
            client.resolve_bucket_name(name).await?
        } else {
            return Err(BucketCatError::NoBucketIdentifier);
        };

        // Create API request
        let request = CatRequest {
            bucket_id,
            path: self.path.clone(),
        };

        // Call API
        let response: CatResponse = client.call(request).await?;

        // Decode base64 content
        let bytes = base64::engine::general_purpose::STANDARD.decode(&response.content)?;

        // Try to convert to UTF-8 string, or show hex if binary
        match String::from_utf8(bytes.clone()) {
            Ok(text) => Ok(format!(
                "File: {}\nSize: {} bytes\n\n{}",
                response.path, response.size, text
            )),
            Err(_) => {
                // Binary content - show hex representation
                let hex = bytes
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                Ok(format!(
                    "File: {}\nSize: {} bytes\nBinary content (hex): {}",
                    response.path, response.size, hex
                ))
            }
        }
    }
}
