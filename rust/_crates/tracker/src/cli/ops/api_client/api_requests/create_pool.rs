use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};

use super::ApiRequest;

#[derive(Debug, Clone, Serialize)]
pub struct CreatePool {
    pub hash: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePoolResponse {
    pub success: bool,
}

impl ApiRequest for CreatePool {
    type Response = CreatePoolResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let url = base_url.join("api/v0/pool").expect("Failed to join URL");
        client.post(url).json(&self)
    }
}
