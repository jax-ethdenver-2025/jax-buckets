use axum::routing::get;
use axum::Router;
use http::header::{ACCEPT, ORIGIN};
use http::Method;
use tower_http::cors::{Any, CorsLayer};

mod bucket_explorer;
mod buckets;
mod file_viewer;
mod index;
mod peers_explorer;
mod pins_explorer;

use crate::ServiceState;

pub fn router(state: ServiceState) -> Router<ServiceState> {
    let cors_layer = CorsLayer::new()
        .allow_methods(vec![Method::GET])
        .allow_headers(vec![ACCEPT, ORIGIN])
        .allow_origin(Any)
        .allow_credentials(false);

    Router::new()
        .route("/", get(index::handler))
        .route("/buckets", get(buckets::handler))
        .route("/buckets/:bucket_id", get(bucket_explorer::handler))
        .route("/buckets/:bucket_id/view", get(file_viewer::handler))
        .route("/buckets/:bucket_id/pins", get(pins_explorer::handler))
        .route("/buckets/:bucket_id/peers", get(peers_explorer::handler))
        .with_state(state)
        .layer(cors_layer)
}
