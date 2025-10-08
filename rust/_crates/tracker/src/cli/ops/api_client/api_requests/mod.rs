mod create_pool;
mod health;
mod list;
mod pools;
mod probe;
mod query;
mod share;

pub use create_pool::{CreatePool, CreatePoolResponse};
pub use health::{Liveness, Readiness};
pub use list::{List, ListResponse};
pub use pools::{Pools, PoolsResponse};
pub use probe::{Probe, ProbeStats};
pub use query::Query;
pub use share::Share;

use reqwest::{Client, RequestBuilder, Url};
use serde::de::DeserializeOwned;

/// Defintion of an API request
pub trait ApiRequest: Send + Sync {
    /// Has a response type
    type Response: DeserializeOwned;

    /// Builds a Reqwest request
    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder;
}
