use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::prelude::MountError;

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
    pub mime_type: String,
}

#[axum::debug_handler]
pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<CatRequest>,
) -> Result<impl IntoResponse, CatError> {
    // Use mount_ops to get file content
    let file_content = crate::mount_ops::get_file_content(req.bucket_id, req.path.clone(), &state)
        .await
        .map_err(|e| match e {
            crate::mount_ops::MountOpsError::BucketNotFound(id) => CatError::BucketNotFound(id),
            crate::mount_ops::MountOpsError::InvalidPath(msg) => CatError::InvalidPath(msg),
            crate::mount_ops::MountOpsError::Mount(me) => CatError::Mount(me),
            e => CatError::MountOps(e.to_string()),
        })?;

    // Encode as base64 for JSON transport
    let content = base64::engine::general_purpose::STANDARD.encode(&file_content.data);
    let size = file_content.data.len();

    Ok((
        http::StatusCode::OK,
        Json(CatResponse {
            path: req.path,
            content,
            size,
            mime_type: file_content.mime_type,
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
    #[error("MountOps error: {0}")]
    MountOps(String),
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
            CatError::MountOps(_) | CatError::Mount(_) => (
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
