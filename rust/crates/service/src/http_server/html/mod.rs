use axum::routing::get;
use axum::Router;
use http::header::{ACCEPT, ORIGIN};
use http::Method;
use tower_http::cors::{Any, CorsLayer};

mod blobs;
mod index;

// use crate::state::ServiceState;

// pub fn router(state: AppState) -> Router<AppState> {
pub fn router() -> Router<()> {
    let cors_layer = CorsLayer::new()
        .allow_methods(vec![Method::GET])
        .allow_headers(vec![ACCEPT, ORIGIN])
        .allow_origin(Any)
        .allow_credentials(false);

    Router::new()
        .route("/", get(index::handler))
        .route("/blobs", get(blobs::handler))
        // .with_state(state)
        .layer(cors_layer)
}
