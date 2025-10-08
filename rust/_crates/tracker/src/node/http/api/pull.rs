use axum::body::Body;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use iroh_blobs::Hash;

use crate::node::State as NodeState;

#[axum::debug_handler]
pub async fn handler(State(state): State<NodeState>, Path(hash): Path<Hash>) -> impl IntoResponse {
    // Pull if needed, then stream
    match state.tracker().pull_blob(hash).await {
        Ok(_) => {
            // Stream the blob
            match state.blobs_service().read_blob(hash).await {
                Ok(reader) => {
                    let body = Body::from_stream(reader);
                    Ok((
                        [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
                        body,
                    ))
                }
                Err(e) => Err((
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to stream blob: {}", e),
                )),
            }
        }
        Err(e) => Err((
            axum::http::StatusCode::NOT_FOUND,
            format!("Failed to get blob: {}", e),
        )),
    }
}
