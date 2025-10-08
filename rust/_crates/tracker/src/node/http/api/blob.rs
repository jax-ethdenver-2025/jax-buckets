use crate::node::State as NodeState;
use axum::{
    extract::{State, HeaderMap},
    http::StatusCode,
    Json,
};
use bytes::Bytes;
use iroh_blobs::BlobFormat;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadResponse {
    hash: String,
}

#[derive(Debug, thiserror::Error)]
pub enum BlobError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid format specified")]
    InvalidFormat,
    #[error("Internal server error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl axum::response::IntoResponse for BlobError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            Self::Io(_) => (StatusCode::INTERNAL_SERVER_ERROR, "IO error"),
            Self::InvalidFormat => (StatusCode::BAD_REQUEST, "Invalid format"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
        };
        (status, message).into_response()
    }
}

pub async fn upload(
    State(state): State<NodeState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<UploadResponse>, BlobError> {
    // Determine format from headers
    let format_str = headers
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("raw");
    
    let format = match format_str {
        "raw" => BlobFormat::Raw,
        "car" => BlobFormat::Car,
        _ => return Err(BlobError::InvalidFormat),
    };
    
    // Store the blob
    let hash = state.blob_service().store_blob(body.to_vec(), format).await?;
    
    // Announce that we have this content
    // This is non-blocking - we don't fail the upload if announcement fails
    let _ = state.tracker_service().announce_content(hash, format).await;
    
    // Return the hash
    Ok(Json(UploadResponse { hash: hash.to_string() }))
}

pub async fn download(
    State(state): State<NodeState>,
    axum::extract::Path(hash_str): axum::extract::Path<String>,
) -> Result<Bytes, BlobError> {
    // Parse the hash
    let hash: iroh_blobs::Hash = hash_str.parse().map_err(|_| BlobError::InvalidFormat)?;
    
    // Try both formats, starting with raw
    if let Ok(data) = state.blob_service().get_blob(hash, BlobFormat::Raw).await {
        return Ok(Bytes::from(data));
    }
    
    if let Ok(data) = state.blob_service().get_blob(hash, BlobFormat::Car).await {
        return Ok(Bytes::from(data));
    }
    
    Err(BlobError::Internal(anyhow::anyhow!("Blob not found")))
} 