use alloy::primitives::Address;
use alloy::primitives::U256;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use crate::node::State as NodeState;

#[derive(Serialize)]
pub struct PoolsResponse {
    pools: Vec<(Address, iroh_blobs::Hash, U256, Vec<(iroh::NodeId, f64)>)>,
    message: String,
}

pub async fn handler(State(state): State<NodeState>) -> Result<impl IntoResponse, PoolsError> {
    let pools = state
        .tracker()
        .list_pools_with_trust()
        .await
        .map_err(PoolsError::Default)?;

    let pools_vec = pools
        .into_iter()
        .map(|(key, trust_scores)| {
            let mut peers = trust_scores.into_iter().collect::<Vec<_>>();
            // Sort peers by trust score (descending)
            peers.sort_by(|(_, a_trust), (_, b_trust)| {
                b_trust
                    .partial_cmp(a_trust)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            (key.key.address, key.key.hash, key.balance, peers)
        })
        .collect::<Vec<_>>();

    let response = PoolsResponse {
        pools: pools_vec.clone(),
        message: format!("Successfully retrieved {} pools", pools_vec.len()),
    };

    Ok((axum::http::StatusCode::OK, Json(response)))
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
