use reqwest::{header::HeaderMap, header::HeaderValue, Client};
use url::Url;
use uuid::Uuid;

use super::error::ApiError;
use super::ApiRequest;
use crate::http_server::api::v0::bucket::list::{ListRequest, ListResponse};

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

    /// Resolve a bucket name to a UUID
    /// Returns the first bucket with an exact name match
    pub async fn resolve_bucket_name(&mut self, name: &str) -> Result<Uuid, ApiError> {
        let request = ListRequest {
            prefix: Some(name.to_string()),
            limit: Some(100),
        };

        let response: ListResponse = self.call(request).await?;

        response
            .buckets
            .into_iter()
            .find(|b| b.name == name)
            .map(|b| b.bucket_id)
            .ok_or_else(|| {
                ApiError::HttpStatus(
                    reqwest::StatusCode::NOT_FOUND,
                    format!("Bucket not found: {}", name),
                )
            })
    }

    /// Get the base URL for API requests
    pub fn base_url(&self) -> &Url {
        &self.remote
    }

    /// Get the underlying HTTP client for custom requests
    pub fn http_client(&self) -> &Client {
        &self.client
    }
}
