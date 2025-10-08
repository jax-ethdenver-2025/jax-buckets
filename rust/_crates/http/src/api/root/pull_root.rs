use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::app::AppState;
use crate::database::models::RootCid;

#[derive(Serialize)]
pub struct PullRootResponse {
    previous_cid: String,
    cid: String,
}

impl From<RootCid> for PullRootResponse {
    fn from(root_cid: RootCid) -> Self {
        PullRootResponse {
            previous_cid: root_cid.previous_cid().to_string(),
            cid: root_cid.cid().to_string(),
        }
    }
}

pub async fn handler(State(state): State<AppState>) -> Result<impl IntoResponse, PullRootError> {
    let db = state.sqlite_database();
    let mut conn = db.acquire().await?;
    let maybe_root_cid = RootCid::pull(&mut conn).await?;
    match maybe_root_cid {
        Some(root_cid) => {
            Ok((http::StatusCode::OK, Json(PullRootResponse::from(root_cid))).into_response())
        }
        None => Err(PullRootError::NotFound),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PullRootError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("root CID error: {0}")]
    RootCid(#[from] crate::database::models::RootCidError),
    #[error("No root CID found")]
    NotFound,
}

impl IntoResponse for PullRootError {
    fn into_response(self) -> Response {
        match self {
            PullRootError::Database(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "unknown server error",
            )
                .into_response(),
            PullRootError::RootCid(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "unknown server error",
            )
                .into_response(),
            PullRootError::NotFound => {
                (http::StatusCode::NOT_FOUND, "No root CID found").into_response()
            }
        }
    }
}
