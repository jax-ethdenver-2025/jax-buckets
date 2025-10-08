use alloy::primitives::U256;
use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use iroh_blobs::ticket::BlobTicket;
use iroh_blobs::BlobFormat;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::node::State as NodeState;

#[derive(Deserialize)]
pub struct ShareRequest {
    path: String,
    initial_value: Option<U256>,
}

#[derive(Serialize)]
pub struct ShareResponse {
    ticket: String,
    hash: String,
    message: String,
}

pub async fn handler(
    State(state): State<NodeState>,
    Json(request): Json<ShareRequest>,
) -> Result<impl IntoResponse, ShareError> {
    // Convert path string to PathBuf and get absolute path
    let path = PathBuf::from(request.path);
    let abs_path = std::path::absolute(&path).map_err(ShareError::Io)?;

    // Read file content
    let content = tokio::fs::read(&abs_path).await.map_err(ShareError::Io)?;

    // Determine format based on file extension
    let format = BlobFormat::Raw;

    // Use blobs_service instead of blob_service
    let hash = state
        .blobs_service()
        .store_blob(content)
        .await
        .map_err(ShareError::BlobOperation)?;

    // Create ticket
    let node_id = state.endpoint().node_id();
    let ticket =
        BlobTicket::new(node_id.into(), hash, format).map_err(ShareError::BlobOperation)?;

    // If initial value is provided, create pool
    if let Some(initial_value) = request.initial_value {
        // Validate user has enough balance
        let eth_address = state.eth_address();
        let tracker = state.tracker();
        let balance = tracker
            .get_address_balance(eth_address)
            .await
            .map_err(ShareError::Default)?;

        if balance < initial_value {
            return Err(ShareError::InsufficientBalance(balance, initial_value));
        }

        // Create pool with initial value
        state
            .tracker()
            .create_pool(hash, Some(initial_value))
            .await
            .map_err(ShareError::Default)?;
    }

    let hash_str = hash.to_string();

    let response = ShareResponse {
        ticket: ticket.to_string(),
        hash: hash_str,
        message: format!(
            "File '{}' has been added to the blob store and announced to the network{}",
            abs_path.display(),
            if request.initial_value.is_some() {
                " with initial pool value"
            } else {
                ""
            }
        ),
    };

    Ok((axum::http::StatusCode::OK, Json(response)))
}

#[derive(Debug, thiserror::Error)]
pub enum ShareError {
    #[error(transparent)]
    Default(#[from] anyhow::Error),
    #[error("io error: {0}")]
    Io(std::io::Error),
    #[error("blob operation failed: {0}")]
    BlobOperation(anyhow::Error),
    #[error("insufficient balance (have {0}, need {1})")]
    InsufficientBalance(U256, U256),
}

impl IntoResponse for ShareError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ShareError::Io(e) => (
                axum::http::StatusCode::BAD_REQUEST,
                format!("File error: {}", e),
            ),
            ShareError::BlobOperation(e) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Blob operation failed: {}", e),
            ),
            ShareError::Default(e) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error: {}", e),
            ),
            ShareError::InsufficientBalance(have, need) => (
                axum::http::StatusCode::BAD_REQUEST,
                format!("Insufficient balance: have {}, need {}", have, need),
            ),
        };

        let body = Json(serde_json::json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
