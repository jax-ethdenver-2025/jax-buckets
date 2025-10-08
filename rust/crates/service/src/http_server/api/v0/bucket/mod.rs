use axum::routing::post;
use axum::Router;

use crate::ServiceState;

pub mod create;
pub mod list;

// Re-export for convenience
pub use create::{CreateRequest, CreateResponse};
pub use list::{ListRequest, ListResponse};

pub fn router(state: ServiceState) -> Router<ServiceState> {
    Router::new()
        .route("/", post(create::handler))
        .route("/list", post(list::handler))
        .with_state(state)
}
