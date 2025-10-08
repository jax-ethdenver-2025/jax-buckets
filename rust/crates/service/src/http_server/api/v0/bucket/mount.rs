use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::prelude::{Link, Mount, MountError};

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct MountRequest {
    /// Bucket ID to mount
    #[cfg_attr(feature = "clap", arg(long))]
    pub bucket_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountResponse {
    pub bucket_id: Uuid,
    pub bucket_name: String,
    pub bucket_link: Link,
    pub root_link: Link,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<MountRequest>,
) -> Result<impl IntoResponse, MountHandlerError> {
    use crate::database::models::Bucket as BucketModel;

    // Get bucket from database
    let bucket = BucketModel::get_by_id(&req.bucket_id, state.database())
        .await
        .map_err(|e| MountHandlerError::Database(e.to_string()))?
        .ok_or_else(|| MountHandlerError::BucketNotFound(req.bucket_id))?;

    // Load mount to verify it works
    let bucket_link: Link = bucket.link.into();
    let secret_key = state.node().secret();
    let blobs = state.node().blobs();

    let mount = Mount::load(&bucket_link, secret_key, blobs).await?;

    // Get root link from mount
    let root_link = mount.inner().link().clone();

    Ok((
        http::StatusCode::OK,
        Json(MountResponse {
            bucket_id: bucket.id,
            bucket_name: bucket.name,
            bucket_link,
            root_link,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum MountHandlerError {
    #[error("Bucket not found: {0}")]
    BucketNotFound(Uuid),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for MountHandlerError {
    fn into_response(self) -> Response {
        match self {
            MountHandlerError::BucketNotFound(id) => (
                http::StatusCode::NOT_FOUND,
                format!("Bucket not found: {}", id),
            )
                .into_response(),
            MountHandlerError::Database(_) | MountHandlerError::Mount(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error".to_string(),
            )
                .into_response(),
        }
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for MountRequest {
    type Response = MountResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/mount").unwrap();
        client.post(full_url).json(&self)
    }
}
