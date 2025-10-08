use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};

use super::ApiRequest;

#[derive(Debug, Clone)]
pub struct List;

#[derive(Debug, Deserialize)]
pub struct BlobInfo {
    pub hash: String,
}

#[derive(Debug, Deserialize)]
pub struct ListResponse {
    pub blobs: Vec<BlobInfo>,
}

impl ApiRequest for List {
    type Response = ListResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let url = base_url.join("api/v0/list").expect("valid URL");
        client.get(url)
    }
}
