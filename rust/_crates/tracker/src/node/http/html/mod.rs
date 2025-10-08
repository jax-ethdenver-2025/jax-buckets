use axum::{routing::get, Router};
use tower_http::services::ServeDir;

use crate::node::State as NodeState;

mod handlers;

pub fn router(state: NodeState) -> Router<NodeState> {
    Router::new()
        .route("/", get(handlers::index_handler))
        .route("/blobs", get(handlers::blobs_handler))
        .route("/pools", get(handlers::pools_handler))
        .route("/share", get(handlers::share_handler))
        .route("/query", get(handlers::query_handler))
        .with_state(state)
        // TODO: make this configurable
        .nest_service("/static", ServeDir::new("crates/node/static"))
}
