use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;
use iroh_blobs::store::ReadableStore;

use crate::node::State as NodeState;

#[derive(Template)]
#[template(path = "blobs.html")]
struct BlobsTemplate {
    blobs: Vec<String>,
}

#[axum::debug_handler]
pub async fn blobs_handler(State(state): State<NodeState>) -> impl IntoResponse {
    let blobs = state
        .blobs_service()
        .get_inner_blobs()
        .store()
        .blobs()
        .await
        .unwrap_or_else(|_| Box::new(std::iter::empty()))
        .filter_map(|r| r.ok())
        .map(|h| h.to_string())
        .collect();

    BlobsTemplate { blobs }
}
