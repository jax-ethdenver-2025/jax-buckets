use axum::extract::{Json, Path, State};
use axum::response::{IntoResponse, Response};
use iroh_blobs::rpc::client::blobs::BlobStatus;
use iroh_blobs::Hash;
use serde::Serialize;

use crate::node::State as NodeState;

#[derive(Serialize)]
pub struct QueryLocationsResponse {
    local: bool,
    nodes: Vec<(iroh::NodeId, f64)>,
    message: String,
}

pub async fn handler(
    State(state): State<NodeState>,
    Path(hash): Path<Hash>,
) -> Result<impl IntoResponse, QueryError> {
    // Get trust scores directly - this already includes peer information
    let trust_scores = state
        .tracker()
        .get_hash_trust(&hash)
        .await
        .map_err(QueryError::Default)?;

    let blob_status = state
        .blobs_service()
        .get_inner_blobs()
        .client()
        .status(hash)
        .await?;
    let local = matches!(blob_status, BlobStatus::Complete { .. });

    let nodes = match trust_scores {
        Some(scores) => scores
            .iter()
            .map(|(key, score)| (*key, *score))
            .collect::<Vec<_>>(),
        None => vec![],
    };
    let response = QueryLocationsResponse {
        local,
        nodes: nodes.clone(),
        message: format!("Found {} nodes hosting blob {}", nodes.len(), hash),
    };

    Ok((axum::http::StatusCode::OK, Json(response)))
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error(transparent)]
    Default(#[from] anyhow::Error),
}

impl IntoResponse for QueryError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            QueryError::Default(e) => (
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
