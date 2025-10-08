use crate::node::State as NodeState;
use axum::{
    extract::{Json, Multipart, State},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use futures::StreamExt;
use iroh_blobs::{ticket::BlobTicket, BlobFormat};
use serde::Serialize;
use std::io;

#[derive(Serialize)]
pub struct ShareResponse {
    ticket: String,
    hash: String,
    message: String,
}

pub async fn handler(
    State(state): State<NodeState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, ShareError> {
    // Get the first field from multipart
    let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(ShareError::Multipart)?
    else {
        return Err(ShareError::NoFile);
    };

    // Read the field into chunks and collect them
    let mut chunks = Vec::new();
    while let Some(chunk) = field.chunk().await.map_err(ShareError::Multipart)? {
        chunks.push(chunk.to_vec());
    }

    // Create a stream from the collected chunks
    let stream = futures::stream::iter(chunks)
        .map(|chunk| Ok(Bytes::from(chunk)))
        .boxed();

    // Use store_stream with our chunk stream
    let hash = state
        .blobs_service()
        .store_stream(stream)
        .await
        .map_err(ShareError::BlobOperation)?;

    // Create ticket
    let node_id = state.endpoint().node_id();
    let ticket = BlobTicket::new(node_id.into(), hash, BlobFormat::Raw)
        .map_err(ShareError::BlobOperation)?;

    let response = ShareResponse {
        ticket: ticket.to_string(),
        hash: hash.to_string(),
        message: format!("File has been streamed to the blob store and announced to the network"),
    };

    Ok((axum::http::StatusCode::OK, Json(response)))
}

#[derive(Debug, thiserror::Error)]
pub enum ShareError {
    #[error(transparent)]
    Default(#[from] anyhow::Error),
    #[error("multipart error: {0}")]
    Multipart(axum::extract::multipart::MultipartError),
    #[error("no file provided")]
    NoFile,
    #[error("blob operation failed: {0}")]
    BlobOperation(anyhow::Error),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

impl IntoResponse for ShareError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ShareError::Multipart(e) => (
                axum::http::StatusCode::BAD_REQUEST,
                format!("Upload error: {}", e),
            ),
            ShareError::NoFile => (
                axum::http::StatusCode::BAD_REQUEST,
                "No file provided".to_string(),
            ),
            ShareError::BlobOperation(e) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Blob operation failed: {}", e),
            ),
            ShareError::Io(e) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("IO error: {}", e),
            ),
            ShareError::Default(e) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error: {}", e),
            ),
        };

        let body = Json(serde_json::json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
