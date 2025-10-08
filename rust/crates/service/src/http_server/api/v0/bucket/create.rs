use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use common::prelude::{Bucket, Mount, MountError};

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct CreateRequest {
    /// Name of the bucket to create
    #[cfg_attr(feature = "clap", arg(long))]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResponse {
    pub bucket_id: Uuid,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<CreateRequest>,
) -> Result<impl IntoResponse, CreateError> {
    use crate::database::models::Bucket as BucketModel;

    // Validate bucket name
    if req.name.is_empty() {
        return Err(CreateError::InvalidName("Name cannot be empty".into()));
    }

    let id = Uuid::new_v4();
    let owner = state.node().secret();
    let blobs = state.node().blobs();
    let mount = Mount::init(id, req.name.clone(), owner, blobs).await?;
    let link = mount.link();

    // Create bucket in database
    let _bucket = BucketModel::create(id, req.name.clone(), link.clone(), state.database())
        .await
        .map_err(|e| match e {
            crate::database::models::bucket::BucketError::AlreadyExists(name) => {
                CreateError::AlreadyExists(name)
            }
            crate::database::models::bucket::BucketError::Database(e) => {
                CreateError::Database(e.to_string())
            }
        })?;

    Ok((
        http::StatusCode::CREATED,
        Json(CreateResponse {
            bucket_id: _bucket.id.clone(),
            name: _bucket.name,
            created_at: _bucket.created_at,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum CreateError {
    #[error("Bucket already exists: {0}")]
    AlreadyExists(String),
    #[error("Invalid bucket name: {0}")]
    InvalidName(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for CreateError {
    fn into_response(self) -> Response {
        match self {
            CreateError::AlreadyExists(name) => (
                http::StatusCode::CONFLICT,
                format!("Bucket already exists: {}", name),
            )
                .into_response(),
            CreateError::InvalidName(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Invalid name: {}", msg),
            )
                .into_response(),
            CreateError::Database(_) | CreateError::Mount(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Unexpected error"),
            )
                .into_response(),
        }
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for CreateRequest {
    type Response = CreateResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket").unwrap();
        client.post(full_url).json(&self)
    }
}
