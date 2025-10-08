use alloy::primitives::Address;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde::Serialize;
use iroh_blobs::Hash;

use crate::node::tracker::PoolKey;
use crate::node::State as NodeState;

#[derive(Deserialize)]
pub struct ClaimRewardsRequest {
    hash: Hash,
    address: Address,
}

#[derive(Serialize)]
pub struct ClaimRewardsResponse {
    message: String,
}

pub async fn handler(
    State(state): State<NodeState>,
    Json(request): Json<ClaimRewardsRequest>,
) -> Result<impl IntoResponse, PoolsError> {
    let tracker = state.tracker();
    let pool_key = PoolKey {
        hash: request.hash,
        address: request.address,
    };
    tracker.claim_pool_rewards(pool_key).await?;
    Ok((axum::http::StatusCode::OK, Json(ClaimRewardsResponse {
        message: "Rewards claimed".to_string(),
    })))
}

#[derive(Debug, thiserror::Error)]
pub enum PoolsError {
    #[error(transparent)]
    Default(#[from] anyhow::Error),
}

impl IntoResponse for PoolsError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            PoolsError::Default(e) => (
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
