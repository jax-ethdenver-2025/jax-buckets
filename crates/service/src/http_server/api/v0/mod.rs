use axum::Router;
use http::header::{ACCEPT, ORIGIN};
use http::Method;
use tower_http::cors::{Any, CorsLayer};

pub mod bucket;

use crate::ServiceState;

pub fn router(state: ServiceState) -> Router<ServiceState> {
    let cors_layer = CorsLayer::new()
        .allow_methods(vec![Method::GET])
        .allow_headers(vec![ACCEPT, ORIGIN])
        .allow_origin(Any)
        .allow_credentials(false);

    Router::new()
        .nest("/bucket", bucket::router(state.clone()))
        .with_state(state)
        .layer(cors_layer)
}
