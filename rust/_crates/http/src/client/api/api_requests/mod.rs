use reqwest::{Client, RequestBuilder, Url};
use serde::de::DeserializeOwned;

mod pull_root;
mod push_root;

pub use pull_root::PullRoot;
pub use push_root::PushRoot;

/// Defintion of an API request
pub trait ApiRequest {
    /// Has a response type
    type Response: DeserializeOwned;

    /// Builds a Reqwest request
    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder;
    /// Optionally requires authentication
    fn requires_authentication(&self) -> bool;
}
