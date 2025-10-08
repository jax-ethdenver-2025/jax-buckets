use alloy::primitives::Address;
use iroh::NodeId;
use iroh_blobs::Hash;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};

use super::ApiRequest;

#[derive(Debug, Clone, Serialize)]
pub struct Pools {}

#[derive(Debug, Clone, Deserialize)]
pub struct PoolsResponse {
    pub pools: Vec<(Address, Hash, Vec<(NodeId, f64)>)>,
    pub message: String,
}

impl ApiRequest for Pools {
    type Response = PoolsResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let url = base_url.join("api/v0/pools").expect("Failed to join URL");
        client.get(url)
    }
}
