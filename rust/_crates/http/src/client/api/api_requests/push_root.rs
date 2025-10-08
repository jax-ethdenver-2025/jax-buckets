use reqwest::{Client, RequestBuilder, Url};
use serde::Deserialize;
use serde::Serialize;

use crate::api::api_requests::ApiRequest;

#[derive(Debug, Serialize)]
pub struct PushRoot {
    pub cid: String,
    pub previous_cid: String,
}

#[derive(Debug, Deserialize)]
pub struct PushRootResponse {
    pub cid: String,
    pub previous_cid: String,
}

impl ApiRequest for PushRoot {
    type Response = PushRootResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/root").unwrap();
        client.post(full_url).json(&self)
    }

    fn requires_authentication(&self) -> bool {
        true
    }
}
