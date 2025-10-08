use std::convert::TryFrom;

use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, Url,
};
use std::fmt::Debug;
use thumbs_up::prelude::{ApiToken, EcKey, PrivateKey, PublicKey};

use super::api_requests::ApiRequest;
use super::error::ApiError;
use crate::ipfs_rpc::IpfsRpc;

/// The audience for the API token
const AUDIENCE: &str = "leaky-server";

#[derive(Debug, Clone)]
/// ApiClient for interacting with our API
pub struct ApiClient {
    /// Base URL for interacting with core service
    pub remote: Url,
    /// Bearer auth
    pub claims: Option<ApiToken>,
    /// Credentials for signing
    pub signing_key: Option<EcKey>,
    /// The current bearer token
    pub bearer_token: Option<String>,
    /// The reqwest client
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
            claims: None,
            signing_key: None,
            bearer_token: None,
            client,
        })
    }

    /// Set the credentials for signing
    /// # Arguments
    /// * `credentials` - The credentials to use for signing
    pub fn with_credentials(&mut self, signing_key: EcKey) {
        self.bearer_token = None;
        self.claims = Some(ApiToken::new(AUDIENCE.to_string(), "leaky".to_string()));
        self.signing_key = Some(signing_key);
    }

    /// Return a bearer token based on the current credentials
    /// # Returns
    /// * `Option<String>` - The bearer token
    /// # Errors
    /// * `ApiClientError` - If there is an error generating the token.
    ///    If the bearer token can not be encoded, or if the signing key is not available.
    pub fn bearer_token(&mut self) -> Result<String, ApiError> {
        match &self.claims {
            Some(claims) => {
                let is_expired = claims.is_expired()?;
                // If we already have a bearer token and the claims are still valid
                // return the current bearer token
                if !is_expired && self.bearer_token.is_some() {
                    return Ok(self.bearer_token.clone().unwrap());
                }
                claims.refresh()?;
                match &self.signing_key {
                    Some(signing_key) => {
                        self.bearer_token = Some(claims.encode_to(signing_key)?);
                        Ok(self.bearer_token.clone().unwrap())
                    }
                    _ => Err(ApiError::AuthRequired),
                }
            }
            // No claims, so no bearer token
            _ => match &self.bearer_token {
                Some(bearer_token) => Ok(bearer_token.clone()),
                _ => Err(ApiError::AuthRequired),
            },
        }
    }

    /// Simple shortcut for checking if a user is authenticated
    pub async fn is_authenticated(&mut self) -> bool {
        self.bearer_token().is_ok()
    }

    /// Call a method that implements ApiRequest on the core server
    pub async fn call<T: ApiRequest>(&mut self, request: T) -> Result<T::Response, ApiError> {
        // Determine if this request requires authentication
        let add_authentication = request.requires_authentication();
        let mut request_builder = request.build_request(&self.remote, &self.client);

        if add_authentication {
            let bearer_token = self.bearer_token()?;
            request_builder = request_builder.bearer_auth(bearer_token);
        }

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

    pub fn ipfs_rpc(&mut self) -> Result<IpfsRpc, ApiError> {
        let url = self.remote.clone();
        let mut client = IpfsRpc::try_from(url)
            .expect("valid ipfs url")
            .with_path("ipfs");
        if self.signing_key.is_some() {
            client = client
                .clone()
                .with_bearer_token(self.bearer_token()?.clone());
            return Ok(client);
        }
        Ok(client)
    }
}
