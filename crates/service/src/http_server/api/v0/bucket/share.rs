use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use common::prelude::Link;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::crypto::PublicKey;

use crate::http_server::api::client::ApiRequest;
use crate::mount_ops::MountOpsError;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct ShareRequest {
    /// Bucket ID to share
    #[cfg_attr(feature = "clap", arg(long))]
    pub bucket_id: Uuid,

    /// Public key of the peer to share with (hex-encoded)
    #[cfg_attr(feature = "clap", arg(long))]
    pub peer_public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareResponse {
    pub bucket_id: Uuid,
    pub peer_public_key: String,
    pub new_bucket_link: String,
}

#[axum::debug_handler]
pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<ShareRequest>,
) -> Result<impl IntoResponse, ShareError> {
    // Parse the peer's public key from hex
    let peer_public_key = PublicKey::from_hex(&req.peer_public_key)
        .map_err(|e| ShareError::InvalidPublicKey(e.to_string()))?;

    // Run file operations in blocking task
    let new_bucket_link = tokio::task::spawn_blocking(move || -> Result<Link, MountOpsError> {
        tokio::runtime::Handle::current().block_on(async {
            tracing::info!("Adding file to mount");
            let bucket_link =
                crate::mount_ops::share_bucket(req.bucket_id, peer_public_key, &state).await?;
            Ok(bucket_link)
        })
    })
    .await
    .map_err(|e| ShareError::Mount(format!("Task join error: {}", e)))??;

    tracing::info!(
        "Bucket {} shared with peer {}",
        req.bucket_id,
        req.peer_public_key
    );

    Ok((
        http::StatusCode::OK,
        Json(ShareResponse {
            bucket_id: req.bucket_id,
            peer_public_key: req.peer_public_key,
            new_bucket_link: new_bucket_link.hash().to_string(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum ShareError {
    #[error("Bucket not found: {0}")]
    BucketNotFound(Uuid),
    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),
    #[error("Share not found")]
    ShareNotFound,
    #[error("Database error: {0}")]
    Database(String),
    #[error("Mount error: {0}")]
    Mount(String),
    #[error("Crypto error: {0}")]
    Crypto(String),
}

impl From<MountOpsError> for ShareError {
    fn from(err: MountOpsError) -> Self {
        match err {
            MountOpsError::BucketNotFound(id) => ShareError::BucketNotFound(id),
            MountOpsError::ShareNotFound => ShareError::ShareNotFound,
            MountOpsError::Database(msg) => ShareError::Database(msg),
            MountOpsError::Mount(e) => ShareError::Mount(e.to_string()),
            MountOpsError::CryptoError(msg) => ShareError::Crypto(msg),
            MountOpsError::ShareError(msg) => ShareError::Crypto(msg),
            MountOpsError::InvalidPath(msg) => ShareError::Mount(msg),
        }
    }
}

impl IntoResponse for ShareError {
    fn into_response(self) -> Response {
        match self {
            ShareError::BucketNotFound(id) => (
                http::StatusCode::NOT_FOUND,
                format!("Bucket not found: {}", id),
            )
                .into_response(),
            ShareError::InvalidPublicKey(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Invalid public key: {}", msg),
            )
                .into_response(),
            ShareError::ShareNotFound => (
                http::StatusCode::NOT_FOUND,
                "Share not found for this bucket".to_string(),
            )
                .into_response(),
            ShareError::Database(_) | ShareError::Mount(_) | ShareError::Crypto(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error".to_string(),
            )
                .into_response(),
        }
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for ShareRequest {
    type Response = ShareResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/share").unwrap();
        client.post(full_url).json(&self)
    }
}
