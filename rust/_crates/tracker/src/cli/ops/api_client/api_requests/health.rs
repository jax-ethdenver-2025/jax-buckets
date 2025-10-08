use alloy::primitives::Address;
use iroh::NodeId;
use reqwest::{Client, RequestBuilder, Url};
use serde::Deserialize;

use super::ApiRequest;

pub struct Liveness;
pub struct Readiness;

#[derive(Debug, Deserialize)]
pub struct LivenessResponse {
    status: String,
}

impl LivenessResponse {
    pub fn status(&self) -> &str {
        &self.status
    }
}

#[derive(Debug, Deserialize)]
pub struct ReadinessResponse {
    pub node_id: NodeId,
    pub eth_address: Address,
}

impl ApiRequest for Liveness {
    type Response = LivenessResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/_status/livez").unwrap();
        client.get(full_url)
    }
}

impl ApiRequest for Readiness {
    type Response = ReadinessResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/_status/readyz").unwrap();
        client.get(full_url)
    }
}
