use alloy::primitives::{Address, U256};
use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use iroh_blobs::Hash;
use serde::{Deserialize, Serialize};

use crate::node::tracker::PoolKey;
use crate::node::State as NodeState;

#[derive(Deserialize)]
pub struct DepositRequest {
    address: Address,
    hash: Hash,
    amount: U256,
}

#[derive(Serialize)]
pub struct DepositResponse {
    success: bool,
    message: String,
}

pub async fn handler(
    State(state): State<NodeState>,
    Json(request): Json<DepositRequest>,
) -> Result<impl IntoResponse, DepositError> {
    let pool_key = PoolKey {
        hash: request.hash,
        address: request.address,
    };

    // Validate user has enough balance
    let eth_address = state.eth_address();
    let tracker = state.tracker();
    let balance = tracker
        .get_address_balance(eth_address)
        .await
        .map_err(DepositError::Default)?;

    if balance < request.amount {
        return Err(DepositError::InsufficientBalance(balance, request.amount));
    }

    // Perform deposit
    tracker
        .deposit_into_pool(pool_key, request.amount)
        .await
        .map_err(DepositError::Default)?;

    let response = DepositResponse {
        success: true,
        message: format!("Successfully deposited {} wei into pool", request.amount),
    };

    Ok((axum::http::StatusCode::OK, Json(response)))
}

#[derive(Debug, thiserror::Error)]
pub enum DepositError {
    #[error(transparent)]
    Default(#[from] anyhow::Error),
    #[error("insufficient balance (have {0}, need {1})")]
    InsufficientBalance(U256, U256),
}

impl IntoResponse for DepositError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            DepositError::Default(e) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to deposit: {}", e),
            ),
            DepositError::InsufficientBalance(have, need) => (
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
