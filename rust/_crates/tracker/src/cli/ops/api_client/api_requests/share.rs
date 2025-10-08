use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};

use super::ApiRequest;

#[derive(Debug, Clone, Serialize)]
pub struct Share {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShareResponse {
    pub ticket: String,
    pub hash: String,
    pub message: String,
}

impl ApiRequest for Share {
    type Response = ShareResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let url = base_url.join("api/v0/share").expect("Failed to join URL");
        client.post(url).json(&self)
    }
}
