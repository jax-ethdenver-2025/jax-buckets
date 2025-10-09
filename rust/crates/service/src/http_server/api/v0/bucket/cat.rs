use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use common::prelude::{Link, Mount, MountError};

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct CatRequest {
    /// Bucket ID to read from
    #[cfg_attr(feature = "clap", arg(long))]
    pub bucket_id: Uuid,

    /// Path in bucket to read
    #[cfg_attr(feature = "clap", arg(long))]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatResponse {
    pub path: String,
    /// Base64-encoded file content
    pub content: String,
    pub size: usize,
}

#[axum::debug_handler]
pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<CatRequest>,
) -> Result<impl IntoResponse, CatError> {
    use crate::database::models::Bucket as BucketModel;

    // Get bucket from database
    let bucket = BucketModel::get_by_id(&req.bucket_id, state.database())
        .await
        .map_err(|e| CatError::Database(e.to_string()))?
        .ok_or_else(|| CatError::BucketNotFound(req.bucket_id))?;

    // Load mount
    let bucket_link: Link = bucket.link.into();
    let secret_key = state.node().secret();
    let blobs = state.node().blobs();

    let mount = Mount::load(&bucket_link, secret_key, blobs).await?;

    // Validate path
    let path = PathBuf::from(&req.path);
    if !path.is_absolute() {
        return Err(CatError::InvalidPath("Path must be absolute".into()));
    }

    // Clone blobs for the blocking task
    let blobs_clone = blobs.clone();
    let path_clone = path.clone();

    // Read file in blocking task to avoid Send issues
    let data = tokio::task::spawn_blocking(move || {
        tokio::runtime::Handle::current().block_on(async {
            mount.cat(&path_clone, &blobs_clone).await
        })
    })
    .await
    .map_err(|e| CatError::Mount(MountError::Default(anyhow::anyhow!(e))))??;

    // Encode as base64 for JSON transport
    let content = base64::engine::general_purpose::STANDARD.encode(&data);
    let size = data.len();

    Ok((
        http::StatusCode::OK,
        Json(CatResponse {
            path: req.path,
            content,
            size,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum CatError {
    #[error("Bucket not found: {0}")]
    BucketNotFound(Uuid),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for CatError {
    fn into_response(self) -> Response {
        match self {
            CatError::BucketNotFound(id) => (
                http::StatusCode::NOT_FOUND,
                format!("Bucket not found: {}", id),
            )
                .into_response(),
            CatError::InvalidPath(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Invalid path: {}", msg),
            )
                .into_response(),
            CatError::Database(_) | CatError::Mount(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error".to_string(),
            )
                .into_response(),
        }
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for CatRequest {
    type Response = CatResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/cat").unwrap();
        client.post(full_url).json(&self)
    }
}
