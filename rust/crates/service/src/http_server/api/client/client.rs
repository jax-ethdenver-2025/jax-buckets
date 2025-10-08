use reqwest::{header::HeaderMap, header::HeaderValue, Client};
use url::Url;

use super::error::ApiError;
use super::ApiRequest;

#[derive(Debug, Clone)]
pub struct ApiClient {
    pub remote: Url,
    client: Client,
}

impl ApiClient {
    pub fn new(remote: &Url) -> Result<Self, ApiError> {
        let mut default_headers = HeaderMap::new();
        default_headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        let client = Client::builder().default_headers(default_headers).build()?;

        Ok(Self {
            remote: remote.clone(),
            client,
        })
    }

    pub async fn call<T: ApiRequest>(&mut self, request: T) -> Result<T::Response, ApiError> {
        let request_builder = request.build_request(&self.remote, &self.client);
        let response = request_builder.send().await?;

        if response.status().is_success() {
            Ok(response.json::<T::Response>().await?)
        } else {
            Err(ApiError::HttpStatus(
                response.status(),
                response.text().await?,
            ))
        }
    }
}
