use std::str::FromStr;

use reqwest::{Client, RequestBuilder, Url};
use serde::Deserialize;

use crate::api::api_requests::ApiRequest;
use crate::types::Cid;

pub struct PullRoot;

#[derive(Debug, Deserialize)]
pub struct PullRootResponse {
    cid: String,
}

impl PullRootResponse {
    pub fn cid(&self) -> Cid {
        Cid::from_str(&self.cid).unwrap()
    }
}

impl ApiRequest for PullRoot {
    type Response = PullRootResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/root").unwrap();
        client.get(full_url)
    }

    fn requires_authentication(&self) -> bool {
        false
    }
}
