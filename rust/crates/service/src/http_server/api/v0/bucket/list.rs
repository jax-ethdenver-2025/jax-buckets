use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use common::prelude::Link;

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct ListRequest {
    /// Optional prefix filter
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "clap", arg(long))]
    pub prefix: Option<String>,

    /// Optional limit
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "clap", arg(long))]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse {
    pub buckets: Vec<BucketInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketInfo {
    pub bucket_id: Uuid,
    pub name: String,
    pub link: Link,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<ListRequest>,
) -> Result<impl IntoResponse, ListError> {
    use crate::database::models::Bucket;

    // List buckets from database
    let buckets = Bucket::list(req.prefix, req.limit, state.database())
        .await
        .map_err(|e| ListError::Database(e.to_string()))?;

    // Convert to response format
    let bucket_infos = buckets
        .into_iter()
        .map(|b| BucketInfo {
            bucket_id: b.id,
            name: b.name,
            link: b.link.into(),
            created_at: b.created_at,
        })
        .collect();

    Ok((
        http::StatusCode::OK,
        Json(ListResponse {
            buckets: bucket_infos,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum ListError {
    #[error("Database error: {0}")]
    Database(String),
}

impl IntoResponse for ListError {
    fn into_response(self) -> Response {
        match self {
            ListError::Database(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "unknown server error",
            )
                .into_response(),
        }
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for ListRequest {
    type Response = ListResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/list").unwrap();
        client.post(full_url).json(&self)
    }
}
