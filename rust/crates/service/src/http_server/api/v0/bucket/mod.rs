use axum::routing::post;
use axum::Router;

use crate::ServiceState;

pub mod add;
pub mod create;
pub mod list;
pub mod ls;
pub mod mount;

// Re-export for convenience
pub use add::{AddRequest, AddResponse};
pub use create::{CreateRequest, CreateResponse};
pub use list::{ListRequest, ListResponse};
pub use ls::{LsRequest, LsResponse};
pub use mount::{MountRequest, MountResponse};

pub fn router(state: ServiceState) -> Router<ServiceState> {
    Router::new()
        .route("/", post(create::handler))
        .route("/list", post(list::handler))
        .route("/add", post(add::handler))
        .route("/ls", post(ls::handler))
        .route("/mount", post(mount::handler))
        .with_state(state)
}
