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
    pub link: Link,
    pub is_dir: bool,
}

#[axum::debug_handler]
pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<LsRequest>,
) -> Result<impl IntoResponse, LsError> {
    use crate::database::models::Bucket as BucketModel;

    // Get bucket from database
    let bucket = BucketModel::get_by_id(&req.bucket_id, state.database())
        .await
        .map_err(|e| LsError::Database(e.to_string()))?
        .ok_or_else(|| LsError::BucketNotFound(req.bucket_id))?;

    // Load mount
    let bucket_link: Link = bucket.link.into();
    let secret_key = state.node().secret();
    let blobs = state.node().blobs();

    let mount = Mount::load(&bucket_link, secret_key, blobs).await?;

    // Determine path to list
    let path = req.path.as_deref().unwrap_or("/");
    let path_buf = PathBuf::from(path);
    let deep = req.deep.unwrap_or(false);

    // Clone blobs for the blocking task
    let blobs_clone = blobs.clone();
    let path_buf_clone = path_buf.clone();

    // List items in blocking task to avoid Send issues
    let items = tokio::task::spawn_blocking(move || {
        tokio::runtime::Handle::current().block_on(async {
            if deep {
                mount.ls_deep(&path_buf_clone, &blobs_clone).await
            } else {
                mount.ls(&path_buf_clone, &blobs_clone).await
            }
        })
    })
    .await
    .map_err(|e| LsError::Mount(MountError::Default(anyhow::anyhow!(e))))??;

    // Convert to response format
    let path_infos = items
        .into_iter()
        .map(|(path, node_link)| PathInfo {
            path: path.to_string_lossy().to_string(),
            link: node_link.link().clone(),
            is_dir: node_link.is_dir(),
        })
        .collect();

    Ok((http::StatusCode::OK, Json(LsResponse { items: path_infos })).into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum LsError {
    #[error("Bucket not found: {0}")]
    BucketNotFound(Uuid),
    #[error("Database error: {0}")]
    Database(String),
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
            LsError::Database(_) | LsError::Mount(_) => (
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
