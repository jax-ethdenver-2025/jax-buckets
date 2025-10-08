use service::http_server::api::client::ApiError;
use service::http_server::api::v0::bucket::add::{AddRequest, AddResponse};
use std::env;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum BucketAddError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Add operation failed: {0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::op::Op for AddRequest {
    type Error = BucketAddError;
    type Output = String;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        // Normalize path to absolute
        let path = PathBuf::from(&self.path);
        let absolute_path = if path.is_absolute() {
            path
        } else {
            env::current_dir()?.join(&path)
        };

        // Create request with absolute path
        let request = AddRequest {
            bucket_id: self.bucket_id,
            path: absolute_path.to_string_lossy().to_string(),
            mount_path: self.mount_path.clone(),
        };

        // Call API
        let mut client = ctx.client.clone();
        let response: AddResponse = client.call(request).await?;

        Ok(format!(
            "Added file to bucket at {} (link: {})",
            response.mount_path,
            response.link.hash()
        ))
    }
}
