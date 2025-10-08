use axum::{extract::DefaultBodyLimit, routing, Router};
use http::header::{ACCEPT, ORIGIN};
use http::Method;
use tower_http::cors::{Any, CorsLayer};

mod create_pool;
mod deposit;
mod list;
mod pools;
mod probe;
mod pull;
mod query;
mod share;
mod claim_rewards;
mod share_stream;

pub use create_pool::handler as create_pool_handler;
pub use deposit::handler as deposit_handler;
pub use list::handler as list_handler;
pub use pools::handler as pools_handler;
pub use probe::handler as probe_handler;
pub use pull::handler as pull_handler;
pub use query::handler as query_handler;
pub use share::handler as share_handler;
pub use share_stream::handler as share_stream_handler;
pub use claim_rewards::handler as claim_rewards_handler;

use crate::node::State as NodeState;

// // Increase the size limit for uploads
// const MAX_UPLOAD_SIZE: usize = 1024 * 1024 * 100; // 100MB, adjust as needed

pub fn router(state: NodeState) -> Router<NodeState> {
    let cors_layer = CorsLayer::new()
        .allow_methods(vec![Method::GET, Method::POST])
        .allow_headers(vec![ACCEPT, ORIGIN])
        .allow_origin(Any)
        .allow_credentials(false);

    Router::new()
        .route("/share", routing::post(share_handler))
        .route("/share-stream", routing::post(share_stream_handler))
        .route("/list", routing::get(list_handler))
        .route("/probe", routing::post(probe_handler))
        .route("/query/:hash", routing::get(query_handler))
        .route("/pool", routing::post(create_pool_handler))
        .route("/pools", routing::get(pools_handler))
        .route("/pull/:hash", routing::get(pull_handler))
        .route("/pool/deposit", routing::post(deposit_handler))
        .route("/rewards", routing::post(claim_rewards_handler))
        .with_state(state)
        .layer(cors_layer)
        // Remove the default body size limit
        .layer(DefaultBodyLimit::disable())
    // Add a multipart limit layer for increased size
    // .layer(axum::extract::multipart::MultipartLimitLayer::new(MAX_UPLOAD_SIZE))
}
