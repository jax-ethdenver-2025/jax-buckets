use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, Url,
};
use std::fmt::Debug;

use super::api_requests::ApiRequest;
use super::error::ApiError;

#[derive(Debug, Clone)]
/// ApiClient for interacting with our API
pub struct ApiClient {
    /// Base URL for interacting with core service
    pub remote: Url,
    client: Client,
}

impl ApiClient {
    /// Create a new ApiClient at a remote endpoint
    /// # Arguments
    /// * `remote` - The base URL for the API
    /// # Returns
    /// * `Self` - The client
    pub fn new(remote: &str) -> Result<Self, ApiError> {
        let mut default_headers = HeaderMap::new();
        default_headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        let client = Client::builder().default_headers(default_headers).build()?;

        Ok(Self {
            remote: Url::parse(remote)?,
            client,
        })
    }

    /// Call a method that implements ApiRequest on the core server
    pub async fn call<T: ApiRequest>(&self, request: T) -> Result<T::Response, ApiError> {
        let request_builder = request.build_request(&self.remote, &self.client);

        // Send the request and obtain the response
        let response = request_builder.send().await?;

        // If the call succeeded
        if response.status().is_success() {
            // Interpret the response as a JSON object
            Ok(response.json::<T::Response>().await?)
        } else {
            Err(ApiError::HttpStatus(
                response.status(),
                response.text().await?,
            ))
        }
    }
}
