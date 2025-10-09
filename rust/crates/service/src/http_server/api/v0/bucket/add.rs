use axum::extract::{Multipart, State};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::PathBuf;
use uuid::Uuid;

use common::prelude::{Link, Mount, MountError};

use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct AddRequest {
    /// Bucket ID to add file to
    #[cfg_attr(feature = "clap", arg(long))]
    pub bucket_id: Uuid,

    /// Path in bucket where file should be mounted
    #[cfg_attr(feature = "clap", arg(long))]
    pub mount_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddResponse {
    pub mount_path: String,
    pub link: Link,
    pub bucket_link: Link,
    pub mime_type: String,
}

#[axum::debug_handler]
pub async fn handler(
    State(state): State<ServiceState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AddError> {
    use crate::database::models::Bucket as BucketModel;

    let mut bucket_id: Option<Uuid> = None;
    let mut mount_path: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;

    // Parse multipart form data
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AddError::MultipartError(e.to_string()))?
    {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "bucket_id" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AddError::MultipartError(e.to_string()))?;
                bucket_id = Some(
                    Uuid::parse_str(&text)
                        .map_err(|_| AddError::InvalidRequest("Invalid bucket_id".into()))?,
                );
            }
            "mount_path" => {
                mount_path = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AddError::MultipartError(e.to_string()))?,
                );
            }
            "file" => {
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| AddError::MultipartError(e.to_string()))?
                        .to_vec(),
                );
            }
            _ => {}
        }
    }

    let bucket_id = bucket_id.ok_or_else(|| AddError::InvalidRequest("bucket_id is required".into()))?;
    let mount_path = mount_path.ok_or_else(|| AddError::InvalidRequest("mount_path is required".into()))?;
    let file_data = file_data.ok_or_else(|| AddError::InvalidRequest("file is required".into()))?;

    // Validate mount path
    let mount_path_buf = PathBuf::from(&mount_path);
    if !mount_path_buf.is_absolute() {
        return Err(AddError::InvalidPath("Mount path must be absolute".into()));
    }

    // Detect MIME type from file extension
    let mime_type = mime_guess::from_path(&mount_path_buf)
        .first_or_octet_stream()
        .to_string();

    tracing::info!("Adding file to bucket {} at {} ({})", bucket_id, mount_path, mime_type);

    // Get bucket from database
    let bucket = BucketModel::get_by_id(&bucket_id, state.database())
        .await
        .map_err(|e| AddError::Database(e.to_string()))?
        .ok_or_else(|| AddError::BucketNotFound(bucket_id))?;

    // Load mount
    let bucket_link: Link = bucket.link.into();
    let secret_key = state.node().secret();
    let blobs = state.node().blobs();

    let mut mount = Mount::load(&bucket_link, secret_key, blobs).await?;

    tracing::info!("Mount loaded");

    // Clone for blocking task
    let blobs_clone = blobs.clone();
    let mount_path_clone = mount_path_buf.clone();
    let secret_key_clone = secret_key.clone();

    // Run file operations in blocking task
    let (new_bucket_link, root_node_link) = tokio::task::spawn_blocking(move || -> Result<(Link, Link), MountError> {
        // Create a cursor from the file data
        let cursor = Cursor::new(file_data);

        tokio::runtime::Handle::current().block_on(async {
            tracing::info!("Adding file to mount");
            mount.add(&mount_path_clone, cursor, &blobs_clone).await?;
            tracing::info!("File added to mount");

            // Save the mount (updates bucket in blobs)
            tracing::info!("Saving mount");
            let bucket_link = mount.save(&secret_key_clone, &blobs_clone).await?;
            tracing::info!("Mount saved with new bucket link");

            let root_link = mount.link();
            Ok((bucket_link, root_link))
        })
    })
    .await
    .map_err(|e| AddError::Mount(MountError::Default(anyhow::anyhow!(e))))??;

    // Update bucket link in database
    bucket
        .update_link(new_bucket_link.clone(), state.database())
        .await
        .map_err(|e| AddError::Database(e.to_string()))?;

    Ok((
        http::StatusCode::OK,
        axum::Json(AddResponse {
            mount_path,
            link: root_node_link,
            bucket_link: new_bucket_link,
            mime_type,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum AddError {
    #[error("Bucket not found: {0}")]
    BucketNotFound(Uuid),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Multipart error: {0}")]
    MultipartError(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for AddError {
    fn into_response(self) -> Response {
        match self {
            AddError::BucketNotFound(id) => (
                http::StatusCode::NOT_FOUND,
                format!("Bucket not found: {}", id),
            )
                .into_response(),
            AddError::InvalidPath(msg) | AddError::InvalidRequest(msg) | AddError::MultipartError(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Bad request: {}", msg),
            )
                .into_response(),
            AddError::Database(_) | AddError::Mount(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error".to_string(),
            )
                .into_response(),
        }
    }
}
