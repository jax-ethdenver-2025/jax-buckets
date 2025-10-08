use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use iroh_blobs::store::ReadableStore;
use serde::Serialize;

use crate::node::State as NodeState;

#[derive(Serialize)]
pub struct BlobInfo {
    hash: String,
}

#[derive(Serialize)]
pub struct ListBlobsResponse {
    blobs: Vec<BlobInfo>,
}

pub async fn handler(State(state): State<NodeState>) -> Result<impl IntoResponse, ListBlobsError> {
    // Get blobs from state
    let blobs = state.blobs_service().get_inner_blobs();

    let blob_list = blobs.store().blobs().await?;

    // Convert to response format
    let blob_infos = blob_list
        .into_iter()
        .filter_map(|blob_result| {
            blob_result.ok().map(|hash| BlobInfo {
                hash: hash.to_string(),
            })
        })
        .collect();

    let response = ListBlobsResponse { blobs: blob_infos };

    Ok((axum::http::StatusCode::OK, Json(response)))
}

#[derive(Debug, thiserror::Error)]
pub enum ListBlobsError {
    #[error(transparent)]
    Default(#[from] anyhow::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl IntoResponse for ListBlobsError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ListBlobsError::Default(_) | ListBlobsError::Io(_) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error".to_string(),
            ),
        };

        let body = Json(serde_json::json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
