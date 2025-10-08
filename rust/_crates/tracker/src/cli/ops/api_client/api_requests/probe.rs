use iroh::NodeId;
use iroh_blobs::Hash;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::ApiRequest;

#[derive(Debug, Clone, Serialize)]
pub struct Probe {
    pub hash: Hash,
    pub node: NodeId,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProbeStats {
    pub elapsed: Duration,
    pub bytes_written: u64,
    pub bytes_read: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProbeResponse {
    pub stats: ProbeStats,
    pub message: String,
}

impl ApiRequest for Probe {
    type Response = ProbeResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let url = base_url.join("api/v0/probe").expect("Failed to join URL");
        client.post(url).json(&self)
    }
}
