use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::prelude::{Link, MountError};

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct LsRequest {
    /// Bucket ID to list
    #[cfg_attr(feature = "clap", arg(long))]
    pub bucket_id: Uuid,

    /// Path in bucket to list (defaults to root)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "clap", arg(long))]
    pub path: Option<String>,

    /// List recursively
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "clap", arg(long))]
    pub deep: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LsResponse {
    pub items: Vec<PathInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathInfo {
    pub path: String,
    pub name: String,
    pub link: Link,
    pub is_dir: bool,
    pub mime_type: String,
}

#[axum::debug_handler]
pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<LsRequest>,
) -> Result<impl IntoResponse, LsError> {
    let deep = req.deep.unwrap_or(false);

    // Use mount_ops to list bucket contents
    let items = crate::mount_ops::list_bucket_contents(
        req.bucket_id,
        req.path,
        deep,
        &state,
    )
    .await
    .map_err(|e| match e {
        crate::mount_ops::MountOpsError::BucketNotFound(id) => LsError::BucketNotFound(id),
        crate::mount_ops::MountOpsError::Mount(me) => LsError::Mount(me),
        e => LsError::MountOps(e.to_string()),
    })?;

    // Convert to response format
    let path_infos = items
        .into_iter()
        .map(|item| PathInfo {
            path: item.path,
            name: item.name,
            link: item.link,
            is_dir: item.is_dir,
            mime_type: item.mime_type,
        })
        .collect();

    Ok((http::StatusCode::OK, Json(LsResponse { items: path_infos })).into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum LsError {
    #[error("Bucket not found: {0}")]
    BucketNotFound(Uuid),
    #[error("MountOps error: {0}")]
    MountOps(String),
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for LsError {
    fn into_response(self) -> Response {
        match self {
            LsError::BucketNotFound(id) => (
                http::StatusCode::NOT_FOUND,
                format!("Bucket not found: {}", id),
            )
                .into_response(),
            LsError::MountOps(_) | LsError::Mount(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error".to_string(),
            )
                .into_response(),
        }
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for LsRequest {
    type Response = LsResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/ls").unwrap();
        client.post(full_url).json(&self)
    }
}
