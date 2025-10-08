use alloy::primitives::Address;
use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use iroh::NodeId;
use iroh_blobs::get::Stats;
use iroh_blobs::Hash;
use serde::{Deserialize, Serialize};

use crate::node::tracker::{PoolKey, ProbeResult, Tracker};
use crate::node::State as NodeState;

#[derive(Deserialize)]
pub struct ProbeRequest {
    hash: Hash,
    node: NodeId,
    address: Option<Address>, // Optional pool address
}

#[derive(Serialize)]
pub struct ProbeResponse {
    stats: Option<Stats>,
    trust_updated: bool,
    message: String,
}

pub async fn handler(
    State(state): State<NodeState>,
    Json(request): Json<ProbeRequest>,
) -> Result<impl IntoResponse, ProbeError> {
    // If pool address is provided, use probe_and_update_trust
    if let Some(address) = request.address {
        let key = PoolKey {
            hash: request.hash,
            address,
        };

        match state
            .tracker()
            .probe_and_update_trust(key, request.node)
            .await?
        {
            ProbeResult::Success(stats) => Ok((
                axum::http::StatusCode::OK,
                Json(ProbeResponse {
                    stats: Some(stats),
                    trust_updated: true,
                    message: "Successfully probed node and updated trust".to_string(),
                }),
            )),
            ProbeResult::Timeout(duration) => Ok((
                axum::http::StatusCode::OK,
                Json(ProbeResponse {
                    stats: None,
                    trust_updated: false,
                    message: format!("Probe timed out after {:?}", duration),
                }),
            )),
            ProbeResult::Error => Ok((
                axum::http::StatusCode::OK,
                Json(ProbeResponse {
                    stats: None,
                    trust_updated: false,
                    message: "Probe failed".to_string(),
                }),
            )),
        }
    } else {
        // Fall back to basic probe_node if no pool address
        let ticket = iroh_blobs::ticket::BlobTicket::new(
            request.node.into(),
            request.hash,
            iroh_blobs::BlobFormat::Raw,
        )?;

        let probe_result = Tracker::probe_node(ticket).await;

        Ok((
            axum::http::StatusCode::OK,
            Json(ProbeResponse {
                stats: match probe_result {
                    ProbeResult::Success(stats) => Some(stats),
                    ProbeResult::Timeout(_) => None,
                    ProbeResult::Error => None,
                },
                trust_updated: false,
                message: "Successfully probed node".to_string(),
            }),
        ))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error(transparent)]
    Default(#[from] anyhow::Error),
}

impl IntoResponse for ProbeError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ProbeError::Default(e) => (
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
