use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use common::prelude::{Link, Mount, MountError};

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct AddRequest {
    /// Bucket ID to add file to
    #[cfg_attr(feature = "clap", arg(long))]
    pub bucket_id: Uuid,

    /// Absolute path to file on filesystem
    #[cfg_attr(feature = "clap", arg(long))]
    pub path: String,

    /// Path in bucket where file should be mounted
    #[cfg_attr(feature = "clap", arg(long))]
    pub mount_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddResponse {
    pub mount_path: String,
    pub link: Link,
    pub bucket_link: Link,
}

#[axum::debug_handler]
pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<AddRequest>,
) -> Result<impl IntoResponse, AddError> {
    use crate::database::models::Bucket as BucketModel;

    // Validate paths
    let fs_path = PathBuf::from(&req.path);
    tracing::info!("Validating path: {}", fs_path.display());
    if !fs_path.is_absolute() {
        tracing::error!("Path must be absolute");
        return Err(AddError::InvalidPath("Path must be absolute".into()));
    }
    tracing::info!("Path is absolute");
    if !fs_path.exists() {
        tracing::error!("Path does not exist");
        return Err(AddError::PathNotFound(req.path.clone()));
    }
    tracing::info!("Path exists");
    if !fs_path.is_file() {
        tracing::error!("Path must be a file");
        return Err(AddError::InvalidPath("Path must be a file".into()));
    }

    let mount_path = PathBuf::from(&req.mount_path);
    tracing::info!("Validating mount path: {}", mount_path.display());
    if !mount_path.is_absolute() {
        tracing::error!("Mount path must be absolute");
        return Err(AddError::InvalidPath("Mount path must be absolute".into()));
    }

    // Get bucket from database
    let bucket = BucketModel::get_by_id(&req.bucket_id, state.database())
        .await
        .map_err(|e| AddError::Database(e.to_string()))?
        .ok_or_else(|| AddError::BucketNotFound(req.bucket_id))?;

    tracing::info!("Bucket loaded");
    tracing::info!("Bucket ID: {}", bucket.id);
    tracing::info!("Bucket name: {}", bucket.name);
    tracing::info!("Bucket link: {:?}", bucket.link);

    // Load mount
    let bucket_link: Link = bucket.link.into();
    let secret_key = state.node().secret();
    let blobs = state.node().blobs();

    let mut mount = Mount::load(&bucket_link, secret_key, blobs).await?;

    tracing::info!("Mount loaded");

    // Clone blobs for the blocking task
    let blobs_clone = blobs.clone();
    let fs_path_clone = fs_path.clone();
    let mount_path_clone = mount_path.clone();

    tracing::info!("Mount path: {:?}", mount_path_clone);
    tracing::info!("fs path: {:?}", fs_path_clone);

    // Run file operations in blocking task to avoid Send issues
    let updated_link = tokio::task::spawn_blocking(move || -> Result<Link, MountError> {
        // Read file from filesystem
        let file = std::fs::File::open(&fs_path_clone)
            .map_err(|e| MountError::Default(anyhow::anyhow!(e)))?;

        tracing::info!("File opened");

        // Add file to mount (this is a blocking operation)
        tokio::runtime::Handle::current().block_on(async {
            tracing::info!("Adding file to mount");
            mount.add(&mount_path_clone, file, &blobs_clone).await?;
            tracing::info!("File added to mount");
            Ok(mount.inner().link().clone())
        })
    })
    .await
    .map_err(|e| AddError::Mount(MountError::Default(anyhow::anyhow!(e))))??;

    // Update bucket in database
    bucket
        .update_link(updated_link.clone(), state.database())
        .await
        .map_err(|e| AddError::Database(e.to_string()))?;

    Ok((
        http::StatusCode::OK,
        Json(AddResponse {
            mount_path: req.mount_path,
            link: updated_link.clone(),
            bucket_link: updated_link,
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
    #[error("Path not found: {0}")]
    PathNotFound(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
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
            AddError::InvalidPath(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Invalid path: {}", msg),
            )
                .into_response(),
            AddError::PathNotFound(path) => (
                http::StatusCode::NOT_FOUND,
                format!("Path not found: {}", path),
            )
                .into_response(),
            AddError::Database(_) | AddError::Io(_) | AddError::Mount(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error".to_string(),
            )
                .into_response(),
        }
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for AddRequest {
    type Response = AddResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/add").unwrap();
        client.post(full_url).json(&self)
    }
}
