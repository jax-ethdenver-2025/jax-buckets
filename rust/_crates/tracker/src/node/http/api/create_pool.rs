use alloy::primitives::U256;
use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use iroh_blobs::Hash;
use serde::{Deserialize, Serialize};

use crate::node::State as NodeState;

#[derive(Deserialize)]
pub struct CreatePoolRequest {
    hash: Hash,
    value: Option<U256>,
}

#[derive(Serialize)]
pub struct CreatePoolResponse {
    success: bool,
    message: String,
}

pub async fn handler(
    State(state): State<NodeState>,
    Json(request): Json<CreatePoolRequest>,
) -> Result<impl IntoResponse, CreatePoolError> {
    // Parse the hash
    let hash = request.hash;
    let value = request.value;

    // TODO: return the pool address and owner address
    // Create a pool using the tracker
    state
        .tracker()
        .create_pool(hash, value)
        .await
        .map_err(CreatePoolError::Default)?;

    // Return the response
    let response = CreatePoolResponse {
        success: true,
        message: format!("Pool created for hash {}", hash),
    };

    Ok((axum::http::StatusCode::OK, Json(response)))
}

#[derive(Debug, thiserror::Error)]
pub enum CreatePoolError {
    #[error(transparent)]
    Default(#[from] anyhow::Error),
}

impl IntoResponse for CreatePoolError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            CreatePoolError::Default(e) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create pool: {}", e),
            ),
        };

        let body = Json(serde_json::json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
