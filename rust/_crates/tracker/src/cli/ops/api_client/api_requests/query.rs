use iroh::NodeId;
use iroh_blobs::Hash;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};

use super::ApiRequest;

#[derive(Debug, Clone, Serialize)]
pub struct Query {
    pub hash: Hash,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QueryResponse {
    pub nodes: Vec<(NodeId, f64)>,
    pub message: String,
}

impl ApiRequest for Query {
    type Response = QueryResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let url = base_url
            .join(&format!("api/v0/query/{}", self.hash))
            .expect("Failed to join URL");
        client.get(url).json(&self)
    }
}
