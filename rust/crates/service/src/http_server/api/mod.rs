use axum::Router;
use http::header::{ACCEPT, CONTENT_TYPE, ORIGIN};
use http::Method;
use tower_http::cors::{Any, CorsLayer};

pub mod client;
pub mod v0;

use crate::ServiceState;

pub fn router(state: ServiceState) -> Router<ServiceState> {
    let cors_layer = CorsLayer::new()
        .allow_methods(vec![Method::GET, Method::POST])
        .allow_headers(vec![ACCEPT, CONTENT_TYPE, ORIGIN])
        .allow_origin(Any)
        .allow_credentials(false);

    Router::new()
        .nest("/v0", v0::router(state.clone()))
        .with_state(state)
        .layer(cors_layer)
}
