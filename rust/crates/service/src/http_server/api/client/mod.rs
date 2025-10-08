#[allow(clippy::module_inception)]
mod client;
mod error;

pub use client::ApiClient;
pub use error::ApiError;

use reqwest::{Client, RequestBuilder, Url};
use serde::de::DeserializeOwned;

pub trait ApiRequest {
    type Response: DeserializeOwned;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder;
}
